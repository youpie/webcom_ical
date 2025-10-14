use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EmailProperties::Table)
                    .if_not_exists()
                    .col(pk_auto(EmailProperties::EmailId))
                    .col(string(EmailProperties::MailFrom))
                    .col(string(EmailProperties::SmtpServer))
                    .col(string(EmailProperties::SmtpUsername))
                    .col(string(EmailProperties::SmtpPassword))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EmailProperties::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum EmailProperties {
    Table,
    EmailId,
    SmtpServer,
    SmtpUsername,
    SmtpPassword,
    MailFrom,
}
