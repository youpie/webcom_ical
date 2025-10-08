use sea_orm_migration::{prelude::*, schema::*};

use crate::m20251006_130009_email::EmailProperties;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(KumaProperties::Table)
                    .if_not_exists()
                    .col(pk_auto(KumaProperties::Id))
                    .col(string(KumaProperties::Domain))
                    .col(integer(KumaProperties::HearbeatRetry))
                    .col(integer(KumaProperties::OfflineMailResendHours))
                    .col(integer(KumaProperties::EmailProperties).default(1))
                    .col(integer(KumaProperties::MailPort))
                    .col(boolean(KumaProperties::UseSsl))
                    .foreign_key(
                        ForeignKey::create().name("email_properties_kuma_fk")
                            .from(KumaProperties::Table, KumaProperties::EmailProperties)
                            .to(EmailProperties::Table, EmailProperties::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KumaProperties::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum KumaProperties {
    Table,
    Id,
    Domain,
    HearbeatRetry,
    OfflineMailResendHours,
    EmailProperties,
    MailPort,
    UseSsl,
}