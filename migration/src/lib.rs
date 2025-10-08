pub use sea_orm_migration::prelude::*;

pub mod m20251006_130009_email;
pub mod m20251006_143409_general_settings;
pub mod m20251006_140009_donation;
pub mod m20251006_141509_kuma;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251006_130009_email::Migration),
            Box::new(m20251006_140009_donation::Migration),
            Box::new(m20251006_141509_kuma::Migration),
            Box::new(m20251006_143409_general_settings::Migration)
        ]
    }
}
