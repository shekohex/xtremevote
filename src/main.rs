#![feature(proc_macro_hygiene, decl_macro)]
#![recursion_limit = "256"]

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;
use crate::templates::{doc, Html};
use diesel::prelude::*;
use exitfailure::ExitFailure;
use failure::Fail;
use rocket::{
  config::{Config, Environment, Value},
  http::Status,
  request::{Form, FromRequest},
  response::Redirect,
  Outcome, Request, State,
};
use rocket_contrib::{database, serve::StaticFiles};
use serde::{Deserialize, Serialize};
use std::{
  collections::HashMap,
  fs::File,
  io::Read,
  net::{IpAddr, Ipv4Addr},
};
use typed_html::html;
mod db;
mod migrations;
mod templates;

#[database("app_database")]
struct AppDatabase(diesel::MysqlConnection);

const NOT_ALLOWED: &str = "You are Not Allowed To View This Page";

/// Defines the different levels for log messages.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Deserialize, Serialize)]
pub enum LoggingLevel {
  /// Only shows errors, warnings, and launch information.
  Critical,
  /// Shows everything except debug and trace information.
  Normal,
  /// Shows everything.
  Debug,
  /// Shows nothing.
  Off,
}

impl From<LoggingLevel> for rocket::config::LoggingLevel {
  fn from(l: LoggingLevel) -> Self {
    use rocket::config::LoggingLevel::*;
    match l {
      LoggingLevel::Critical => Critical,
      LoggingLevel::Debug => Debug,
      LoggingLevel::Normal => Normal,
      LoggingLevel::Off => Off,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AppConfig {
  server_name: String,
  allowed_ips: Vec<IpAddr>,
  site_id: u32,
  vote_config: VoteConfig,
  database_config: DatabaseConfig,
  port: u16,
  ip: Ipv4Addr,
  workers: u16,
  log_level: LoggingLevel,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct VoteConfig {
  time_limit: u8,
  points: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DatabaseConfig {
  host: String,
  port: u16,
  username: String,
  password: String,
  database_name: String,
}

#[derive(Debug, Fail, Clone)]
#[fail(display = "Application Error")]
enum AppErrors {
  #[fail(
    display = "Can't Find the config file `config.toml`, create it if it's not exist"
  )]
  ConfigFileNotFound,

  #[fail(display = "Parsing Config file: {}", _0)]
  ConfigParseError(String),

  #[fail(display = "Client IP Notfound")]
  ClientIPNotFound,

  #[fail(display = "DatabaseError: {:?}", _0)]
  DatabaseError(String),

  #[fail(display = "You are Not Ready for vote yet, come later.")]
  NotReadyForVote,
}

#[derive(FromForm, Debug)]
struct VotePostback {
  votingip: Ipv4Addr,
  custom: String,
}

struct AllowedIPs(IpAddr);

#[derive(Responder)]
enum VoteResult {
  #[response(status = 200)]
  Success(()),
  #[response(status = 401)]
  IPNotAllowed(&'static str),
  #[response(status = 401)]
  NotReadyForVote(&'static str),
}

impl<'a, 'r> FromRequest<'a, 'r> for AllowedIPs {
  type Error = AppErrors;

  fn from_request(
    request: &'a Request<'r>,
  ) -> Outcome<Self, (Status, AppErrors), ()> {
    if let Some(client_ip) = request.client_ip() {
      Outcome::Success(Self(client_ip))
    } else {
      Outcome::Failure((
        Status::new(401, "Client IP Not Found"),
        AppErrors::ClientIPNotFound,
      ))
    }
  }
}

#[get("/")]
fn index(config: State<AppConfig>) -> Html {
  Html(doc(
    &config.server_name,
    html!(
          <section class="hero is-success is-fullheight">
            <div class="hero-body">
              <div class="container has-text-centered">
                <div class="column is-4 is-offset-4">
                  <h3 class="title has-text-grey">"Login"</h3>
                  <p class="subtitle has-text-grey">"Please login to proceed."</p>
                  <div class="box">
                    <form onsubmit="return false;">
                      <div class="field">
                        <div class="control">
                          <input
                            class="input is-large"
                            type="text"
                            id="username"
                            autofocus={true}
                            required={true}
                          />
                        </div>
                      </div>
                      <button id="vote" class="button is-block is-info is-large is-fullwidth">
                        <i class="fas fa-vote-yea"></i> " Vote"
                      </button>
                    </form>
                  </div>
                </div>
              </div>
            </div>
          </section>
    ),
  ))
}

#[get("/postback?<data..>")]
fn postback(
  guard: AllowedIPs,
  data: Form<VotePostback>,
  config: State<AppConfig>,
  conn: AppDatabase,
) -> VoteResult {
  if !config.allowed_ips.contains(&guard.0) {
    println!("Got Not allowed IP {}", guard.0);
    return VoteResult::IPNotAllowed(NOT_ALLOWED);
  }
  if let Err(e) = vote(
    &conn,
    &data,
    config.vote_config.points,
    config.vote_config.time_limit,
  ) {
    eprintln!("{}\ncurrent data: {:?}", e, &data.into_inner());
    VoteResult::NotReadyForVote("Your Account Is Not ready for vote yet")
  } else {
    VoteResult::Success(())
  }
}

#[get("/vote/<username>")]
fn forword_to_xtream(username: String, config: State<AppConfig>) -> Redirect {
  // TODO: Check for the username.

  Redirect::to(format!(
    "http://www.xtremetop100.com/in.php?site={}&postback={}",
    config.site_id, username
  ))
}

fn vote(
  conn: &AppDatabase,
  data: &VotePostback,
  points_per_vote: u64,
  time_limit: u8,
) -> Result<(), AppErrors> {
  use db::{
    models::{UVotes, UserVote},
    uvotes::dsl::*,
  };
  if let Ok(mut current_user) = uvotes
    .filter(username.eq(&data.custom))
    .first::<UVotes>(&conn.0)
  {
    // we have an old user, we need to update it.
    // but first, check for last vote time!
    let current_time = chrono::Local::now().naive_local();
    let diff = current_time - current_user.last_vote;
    let limit = chrono::Duration::hours(i64::from(time_limit));
    if diff < limit {
      return Err(AppErrors::NotReadyForVote);
    }
    current_user.points += points_per_vote as u32;
    diesel::update(uvotes.filter(username.eq(&data.custom)))
      .set(points.eq(current_user.points))
      .execute(&conn.0)
      .map_err(|e| AppErrors::DatabaseError(e.to_string()))?;
  } else {
    // It is a new Vote User
    let new_user = UserVote {
      username: data.custom.to_owned(),
      votingip: data.votingip.to_string(),
      points: points_per_vote as u32,
    };
    diesel::insert_into(uvotes)
      .values(new_user)
      .execute(&conn.0)
      .map_err(|e| AppErrors::DatabaseError(e.to_string()))?;
  }
  Ok(())
}

fn main() -> Result<(), ExitFailure> {
  // disable colors and emoji
  std::env::set_var("ROCKET_CLI_COLORS", "off");
  let mut config_file = String::new();
  File::open("./config.toml")
    .map_err(|_| AppErrors::ConfigFileNotFound)?
    .read_to_string(&mut config_file)?;

  let config: AppConfig = toml::from_str(&config_file)
    .map_err(|e| AppErrors::ConfigParseError(format!("{}", e)))?;
  let mut database_config = HashMap::new();
  let mut databases = HashMap::new();
  let DatabaseConfig {
    host,
    port,
    username,
    password,
    database_name,
  } = &config.database_config;
  database_config.insert(
    "url",
    Value::from(format!(
      "mysql://{}:{}@{}:{}/{}",
      username, password, host, port, database_name
    )),
  );
  databases.insert("app_database", Value::from(database_config));
  let dbconfig = Config::build(Environment::active().unwrap())
    .address(config.ip.to_string())
    .port(config.port)
    .workers(config.workers)
    .log_level(config.log_level.into())
    .extra("databases", databases)
    .finalize()?;

  let server = rocket::custom(dbconfig)
    .attach(AppDatabase::fairing())
    .mount("/", routes![index, postback, forword_to_xtream])
    .mount("/assets", StaticFiles::from("/assets"))
    .manage(config);

  // Get database Connection.
  if let Some(conn) = AppDatabase::get_one(&server) {
    // Run Migiration.
    migrations::run_with_output(&conn.0, &mut std::io::stdout())?;
  } else {
    panic!("Error Getting database Connection !");
  }
  server.launch();
  Ok(())
}
