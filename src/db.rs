table! {
    uvotes(id) {
        id -> Integer,
        username -> Text,
        votingip -> Text,
        points -> Integer,
        last_vote -> Timestamp,
    }
}

pub mod models {
  use super::uvotes;
  use chrono::NaiveDateTime;
  #[derive(Queryable, PartialEq, Debug)]
  pub struct UVotes {
    pub id: i32,
    pub username: String,
    pub votingip: String,
    pub points: i32,
    pub last_vote: NaiveDateTime,
  }

  #[derive(Debug, Insertable)]
  #[table_name = "uvotes"]
  pub struct UserVote {
    pub username: String,
    pub votingip: String,
    pub points: i32,
    pub last_vote: NaiveDateTime,
  }
}
