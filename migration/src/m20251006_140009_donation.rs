use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DonationText::Table)
                    .if_not_exists()
                    .col(pk_auto(DonationText::DonationId))
                    .col(string(DonationText::DonateLink))
                    .col(string(DonationText::DonateServiceName))
                    .col(string(DonationText::DonateText))
                    .col(string(DonationText::Iban))
                    .col(string(DonationText::IbanName))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DonationText::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum DonationText {
    Table,
    DonationId,
    DonateLink,
    DonateText,
    DonateServiceName,
    Iban,
    IbanName,
}
