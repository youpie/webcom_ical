use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

use super::m20251006_130009_email::EmailProperties;
use super::m20251006_141509_kuma::KumaProperties;
use super::m20251006_140009_donation::DonationText;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EmailProperties::Table)
                    .if_not_exists()
                    .col(pk_auto(EmailProperties::Id))
                    .col(string(EmailProperties::MailFrom))
                    .col(string(EmailProperties::SmtpServer))
                    .col(string(EmailProperties::SmtpUsername))
                    .col(string(EmailProperties::SmtpPassword))
                    .to_owned()
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(DonationText::Table)
                    .if_not_exists()
                    .col(pk_auto(DonationText::Id))
                    .col(string(DonationText::DonateLink))
                    .col(string(DonationText::DonateServiceName))
                    .col(string(DonationText::DonateText))
                    .col(string(DonationText::Iban))
                    .col(string(DonationText::IbanName))
                    .to_owned()
            )
            .await?;
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
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GeneralPropertiesDB::Table)
                    .if_not_exists()
                    .col(pk_auto(GeneralPropertiesDB::Id))
                    .col(string(GeneralPropertiesDB::SaveTarget))
                    .col(string(GeneralPropertiesDB::IcalDomain))
                    .col(string(GeneralPropertiesDB::WebcalDomain))
                    .col(string(GeneralPropertiesDB::PDFShiftDomain))
                    .col(integer(GeneralPropertiesDB::SigninFailExecutionReduce))
                    .col(integer(GeneralPropertiesDB::SigninFailMailReduce))
                    .col(integer(GeneralPropertiesDB::ExecutionIntervalMinutes))
                    .col(integer(GeneralPropertiesDB::ExpectedExectutionTimeSeconds))
                    .col(integer(GeneralPropertiesDB::ExecutionRetryCount))
                    .col(string(GeneralPropertiesDB::SupportMail))
                    .col(string(GeneralPropertiesDB::PasswordResetLink))
                    .col(integer(GeneralPropertiesDB::KumaProperties).default(1))
                    .col(integer(GeneralPropertiesDB::EmailProperties).default(1))
                    .col(integer(GeneralPropertiesDB::DonationText).default(1))
                    .foreign_key(
                        ForeignKey::create().name("kuma_properties_fk")
                            .from(GeneralPropertiesDB::Table, GeneralPropertiesDB::KumaProperties)
                            .to(KumaProperties::Table, KumaProperties::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade))
                    .foreign_key(
                        ForeignKey::create().name("email_properties_general_fk")
                            .from(GeneralPropertiesDB::Table, GeneralPropertiesDB::EmailProperties)
                            .to(EmailProperties::Table, EmailProperties::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade))
                    .foreign_key(
                        ForeignKey::create().name("donation_text_fk")
                            .from(GeneralPropertiesDB::Table, GeneralPropertiesDB::DonationText)
                            .to(DonationText::Table, DonationText::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GeneralPropertiesDB::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(KumaProperties::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DonationText::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(EmailProperties::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GeneralPropertiesDB {
    Table,
    Id,
    SaveTarget,
    IcalDomain,
    WebcalDomain,
    PDFShiftDomain,
    SigninFailExecutionReduce,
    SigninFailMailReduce,
    ExecutionIntervalMinutes,
    ExpectedExectutionTimeSeconds,
    ExecutionRetryCount,
    SupportMail,
    PasswordResetLink,
    EmailProperties,
    KumaProperties,
    DonationText,
}