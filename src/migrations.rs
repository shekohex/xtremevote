use diesel_migrations::{
  run_migrations, Migration, MigrationConnection, RunMigrationsError,
};
use rocket_contrib::databases::diesel::connection::SimpleConnection;

const ALL_MIGRATIONS: &[&Migration] = &[&EmbeddedMigration {
  version: "20190203204346",
  up_sql: include_str!("./uvotes.sql"),
}];

struct EmbeddedMigration {
  version: &'static str,
  up_sql: &'static str,
}

impl Migration for EmbeddedMigration {
  fn version(&self) -> &str { self.version }

  fn run(&self, conn: &SimpleConnection) -> Result<(), RunMigrationsError> {
    conn.batch_execute(self.up_sql).map_err(Into::into)
  }

  fn revert(&self, _conn: &SimpleConnection) -> Result<(), RunMigrationsError> {
    unreachable!()
  }
}

pub fn run<C: MigrationConnection>(conn: &C) -> Result<(), RunMigrationsError> {
  run_with_output(conn, &mut std::io::sink())
}

pub fn run_with_output<C: MigrationConnection>(
  conn: &C,
  out: &mut std::io::Write,
) -> Result<(), RunMigrationsError> {
  run_migrations(conn, ALL_MIGRATIONS.iter().cloned(), out)
}
