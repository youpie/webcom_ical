use arc_swap::ArcSwap;
use entity::{
    donation_text, email_properties, general_properties_db, kuma_properties, user_data,
    user_properties,
};
use sea_orm::RelationTrait;
use sea_orm::{ColumnTrait, QuerySelect};
use sea_orm::{DatabaseConnection, DerivePartialModel, EntityTrait, QueryFilter};

use crate::GenResult;

const DEFAULT_PREFERENCES_ID: i32 = 1;

#[derive(Debug)]
pub struct UserInstance {
    pub user_data: ArcSwap<UserData>,
    pub general_settings: ArcSwap<GeneralProperties>,
}

impl UserInstance {
    pub async fn load_user(db: &DatabaseConnection, username: &str) -> GenResult<Option<Self>> {
        let userdata = UserData::get_username(db, username).await?;
        if let Some(user_data) = userdata {
            let custom_properties_id = user_data.custom_general_properties.clone();
            Ok(Some(Self {
                user_data: ArcSwap::from_pointee(user_data),
                general_settings: ArcSwap::from_pointee(
                    Self::load_preferences(db, custom_properties_id).await?,
                ),
            }))
        } else {
            Ok(None)
        }
    }
    async fn load_preferences(
        db: &DatabaseConnection,
        custom_id: Option<i32>,
    ) -> GenResult<GeneralProperties> {
        if let Some(id) = custom_id
            && let Some(custom_properties) = GeneralProperties::get(db, id).await?
        {
            Ok(custom_properties)
        } else {
            Ok(GeneralProperties::get(db, DEFAULT_PREFERENCES_ID)
                .await?
                .expect("No default preferences"))
        }
    }
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "general_properties_db::Entity")]
pub struct GeneralProperties {
    general_properties_id: i32,
    save_target: String,
    ical_domain: String,
    webcal_domain: String,
    pdf_shift_domain: String,
    signin_fail_execution_reduce: i32,
    signin_fail_mail_reduce: i32,
    execution_interval_minutes: i32,
    expected_execution_time_seconds: i32,
    execution_retry_count: i32,
    support_mail: String,
    password_reset_link: String,
    #[sea_orm(nested)]
    kuma_properties: KumaProperties,
    #[sea_orm(from_col = "general_email_properties")]
    email_id: i32,
    #[sea_orm(nested, alias = "general_email")]
    general_email_properties: email_properties::Model,
    #[sea_orm(nested)]
    donation_text: DonationText,
}

impl GeneralProperties {
    pub async fn get(db: &DatabaseConnection, id: i32) -> GenResult<Option<GeneralProperties>> {
        Ok(general_properties_db::Entity::find_by_id(id)
            .left_join(kuma_properties::Entity)
            .left_join(email_properties::Entity)
            .left_join(donation_text::Entity)
            .join_as(
                sea_orm::JoinType::LeftJoin,
                kuma_properties::Relation::EmailProperties.def(),
                "kuma_email",
            )
            .join_as(
                sea_orm::JoinType::LeftJoin,
                general_properties_db::Relation::EmailProperties.def(),
                "general_email",
            )
            .into_partial_model()
            .one(db)
            .await?)
    }
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "email_properties::Entity")]
struct EmailProperties {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "kuma_properties::Entity")]
pub struct KumaProperties {
    domain: String,
    hearbeat_retry: i32,
    offline_mail_resend_hours: i32,
    #[sea_orm(from_col = "kuma_email_properties")]
    email_id: i32,
    #[sea_orm(nested, alias = "kuma_email")]
    kuma_email_properties: email_properties::Model,
    mail_port: i32,
    use_ssl: bool,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "donation_text::Entity")]
struct DonationText {
    donate_link: String,
    donate_text: String,
    donate_service_name: String,
    iban: String,
    iban_name: String,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "user_data::Entity")]
pub struct UserData {
    pub personeelsnummer: String,
    pub password: String,
    pub email: String,
    pub file_name: String,
    #[sea_orm(nested)]
    pub user_properties: user_properties::Model,
    custom_general_properties: Option<i32>,
}

impl UserData {
    pub async fn get_username(db: &DatabaseConnection, username: &str) -> GenResult<Option<Self>> {
        if let Some(id) = user_data::Entity::find()
            .filter(user_data::Column::UserName.contains(username))
            .column(user_data::Column::UserDataId)
            .into_tuple::<i32>()
            .one(db)
            .await?
        {
            UserData::get_id(db, id).await
        } else {
            Ok(None)
        }
    }
    pub async fn get_id(db: &DatabaseConnection, id: i32) -> GenResult<Option<Self>> {
        Ok(user_data::Entity::find_by_id(id)
            .left_join(user_properties::Entity)
            .left_join(general_properties_db::Entity)
            .join_as(
                sea_orm::JoinType::LeftJoin,
                general_properties_db::Relation::EmailProperties.def(),
                "general_email",
            )
            .into_partial_model::<UserData>()
            .one(db)
            .await?)
    }
}
