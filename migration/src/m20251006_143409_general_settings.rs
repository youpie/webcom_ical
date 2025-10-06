use sea_orm_migration::{prelude::*, schema::*};

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
                    .col(string(KumaProperties::SMTPServer))
                    .col(string(KumaProperties::SMTPUsername))
                    .col(string(KumaProperties::SMTPPassword))
                    .col(string(KumaProperties::MailFrom))
                    .col(integer(KumaProperties::MailPort))
                    .col(boolean(KumaProperties::UseSsl))
                    .col(integer(KumaProperties::HearbeatRetry))
                    .col(integer(KumaProperties::OfflineMailResendHours))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GeneralProperties::Table)
                    .if_not_exists()
                    .col(pk_auto(GeneralProperties::Id))
                    .col(string(GeneralProperties::SaveTarget))
                    .col(string(GeneralProperties::WebcalDomain))
                    .col(string(GeneralProperties::PDFShiftDomain))
                    .col(integer(GeneralProperties::SigninFailExecutionReduce))
                    .col(integer(GeneralProperties::SigninFailMailReduce))
                    .col(integer(GeneralProperties::ExecutionIntervalMinutes))
                    .col(integer(GeneralProperties::ExpectedExectutionTimeSeconds))
                    .col(integer(GeneralProperties::ExecutionRetryCount))
                    .col(integer(GeneralProperties::KumaProperties).default(1))
                    .foreign_key(
                        ForeignKey::create().name("kuma_properties_fk")
                            .from(GeneralProperties::Table, GeneralProperties::KumaProperties)
                            .to(KumaProperties::Table, KumaProperties::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KumaProperties::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GeneralProperties::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GeneralProperties {
    Table,
    Id,
    SaveTarget,
    WebcalDomain,
    PDFShiftDomain,
    SigninFailExecutionReduce,
    SigninFailMailReduce,
    ExecutionIntervalMinutes,
    ExpectedExectutionTimeSeconds,
    ExecutionRetryCount,
    // EmailProperties,
    KumaProperties,
    // DonationText,
}

#[derive(DeriveIden)]
enum KumaProperties {
    Table,
    Id,
    Domain,
    SMTPServer,
    SMTPUsername,
    SMTPPassword,
    MailFrom,
    MailPort,
    UseSsl,
    HearbeatRetry,
    OfflineMailResendHours,
}
