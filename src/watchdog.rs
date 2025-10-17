use std::collections::HashMap;

use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::{
    GenResult,
    variables::{ArcUserInstanceData, UserData},
};

pub struct UserInstance {
    pub user_instance_data: ArcUserInstanceData,
    pub thread_handle: JoinHandle<GenResult<()>>,
}

type InstanceName = String;

pub async fn watchdog(db: &DatabaseConnection) -> GenResult<()> {
    let instances: HashMap<InstanceName, UserInstance> = HashMap::new();
    let users = UserData::get_all_usernames(db).await?;
    info!("Users: {users:#?}");
    Ok(())
}
