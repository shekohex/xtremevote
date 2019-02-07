#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;
use diesel::{connection::SimpleConnection, prelude::*};
use exitfailure::ExitFailure;
use failure::Fail;
use rocket::{
  config::{Config, Environment, Value},
  http::Status,
  request::{Form, FromRequest},
  response::{content::Html, Redirect},
  Outcome, Request, State,
};
use rocket_contrib::database;
use serde::{Deserialize, Serialize};
use std::{
  collections::HashMap,
  fmt,
  fs::File,
  io::Read,
  net::{IpAddr, Ipv4Addr},
  ops::Deref,
};
mod db;

#[database("app_database")]
struct AppDatabase(diesel::SqliteConnection);

const NOT_ALLOWED: &str = "You are Not Allowed To View This Page";
const TEMPLATE: &str = include_str!("./template.html");
const MIGRATIONS: &str = include_str!("./uvotes.sql");

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
  port: u16,
  ip: Ipv4Addr,
  workers: u16,
  log_level: LoggingLevel,
  database_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct VoteConfig {
  time_limit: u8,
  points: u64,
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

#[derive(Debug, PartialEq, Eq)]
struct ClientIP(IpAddr);

impl Deref for ClientIP {
  type Target = IpAddr;

  fn deref(&self) -> &Self::Target { &self.0 }
}

impl fmt::Display for ClientIP {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Responder)]
enum VoteResult {
  #[response(status = 200)]
  Success(()),
  #[response(status = 401)]
  IPNotAllowed(&'static str),
  #[response(status = 401)]
  NotReadyForVote(&'static str),
}

#[derive(Responder)]
enum ControllerResult {
  #[response(status = 200)]
  Success(String),
  #[response(status = 401)]
  IPNotAllowed(&'static str),
  #[response(status = 500)]
  DatabaseError(&'static str),
}

impl<'a, 'r> FromRequest<'a, 'r> for ClientIP {
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
fn index(config: State<AppConfig>) -> Html<String> {
  let server_name = &config.server_name;
  Html(TEMPLATE.replace("APP_TITLE", server_name))
}

#[get("/postback?<data..>")]
fn postback(
  client_ip: ClientIP,
  data: Form<VotePostback>,
  config: State<AppConfig>,
  conn: AppDatabase,
) -> VoteResult {
  println!("Got POSTBACK Request from IP {}", client_ip);
  if !config.allowed_ips.contains(&client_ip) {
    println!("Got Not allowed IP {}", client_ip);
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

#[get("/points?<u>")]
fn get_user_points(
  client_ip: ClientIP,
  conn: AppDatabase,
  u: String,
) -> ControllerResult {
  use db::uvotes::dsl::{points, username, uvotes};
  if client_ip.0 != Ipv4Addr::from([127, 0, 0, 1]) {
    ControllerResult::IPNotAllowed("You Cant't view this page.")
  } else {
    ControllerResult::Success(
      uvotes
        .filter(username.eq(&u))
        .select(points)
        .first::<i32>(&conn.0)
        .unwrap_or_default()
        .to_string(),
    )
  }
}

#[post("/points?<u>&<p>")]
fn update_user_points(
  client_ip: ClientIP,
  conn: AppDatabase,
  u: String,
  p: i32,
) -> ControllerResult {
  use db::uvotes::dsl::{points, username, uvotes};
  if client_ip.0 != Ipv4Addr::from([127, 0, 0, 1]) {
    ControllerResult::IPNotAllowed("You Cant't view this page.")
  } else if let Err(e) = diesel::update(uvotes.filter(username.eq(&u)))
    .set(points.eq(p))
    .execute(&conn.0)
  {
    eprintln!("{}", e);
    ControllerResult::DatabaseError("Error While Updating That user.")
  } else {
    ControllerResult::Success(p.to_string())
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
    uvotes::dsl::{points, username, uvotes, votingip},
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
    if diff < limit && data.votingip.to_string() == current_user.votingip {
      return Err(AppErrors::NotReadyForVote);
    }
    current_user.points += points_per_vote as i32;
    diesel::update(uvotes.filter(username.eq(&data.custom)))
      .set((
        votingip.eq(data.votingip.to_string()),
        points.eq(current_user.points),
      ))
      .execute(&conn.0)
      .map_err(|e| AppErrors::DatabaseError(e.to_string()))?;
  } else {
    let current_time = chrono::Local::now().naive_local();
    // It is a new Vote User
    let new_user = UserVote {
      username: data.custom.to_owned(),
      votingip: data.votingip.to_string(),
      points: points_per_vote as i32,
      last_vote: current_time,
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
  let mut database_config: HashMap<&str, &str> = HashMap::new();
  let mut databases = HashMap::new();
  database_config.insert("url", &config.database_url);
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
    .mount(
      "/",
      routes![
        index,
        postback,
        forword_to_xtream,
        get_user_points,
        update_user_points
      ],
    )
    .manage(config);

  // Get database Connection.
  println!("Testing Database Connection..");
  if let Some(conn) = AppDatabase::get_one(&server) {
    // Run Migiration.
    println!("Checking for `uvotes` table.");
    conn.batch_execute(MIGRATIONS)?;
    println!("Database is OK !");
  } else {
    panic!("Error Getting database Connection !");
  }
  server.launch();
  Ok(())
}
