use sea_orm_migration::{prelude::*, schema::*};

use crate::m20251008_194017_user_settings::UserProperties;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserData::Table)
                    .if_not_exists()
                    .col(pk_auto(UserData::UserDataId))
                    .col(string(UserData::Personeelsnummer).not_null())
                    .col(string(UserData::Password))
                    .col(string(UserData::Email))
                    .col(string(UserData::FileName))
                    .col(integer(UserData::UserProperties).not_null())
                    .col(integer(UserData::CustomGeneralProperties).null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_properties_fk")
                            .from(UserData::Table, UserData::UserProperties)
                            .to(UserProperties::Table, UserProperties::UserPropertiesId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserData::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserData {
    Table,
    UserDataId,
    Personeelsnummer,
    Password,
    Email,
    FileName,
    UserProperties,
    CustomGeneralProperties,
}
