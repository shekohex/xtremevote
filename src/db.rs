table! {
    uvotes {
        id -> Unsigned<Integer>,
        username -> VarChar,
        votingip -> VarChar,
        points -> Unsigned<Integer>,
        last_vote -> Timestamp,
    }
}

pub mod models {
  use super::uvotes;
  use chrono::NaiveDateTime;
  #[derive(Queryable, PartialEq, Debug)]
  pub struct UVotes {
    pub id: u32,
    pub username: String,
    pub votingip: String,
    pub points: u32,
    pub last_vote: NaiveDateTime,
  }

  #[derive(Debug, Insertable)]
  #[table_name = "uvotes"]
  pub struct UserVote {
    pub username: String,
    pub votingip: String,
    pub points: u32,
  }
}
