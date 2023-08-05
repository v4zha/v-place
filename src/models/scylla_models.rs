use chrono::Utc;
use scylla::prepared_statement::PreparedStatement;
use scylla::transport::query_result::FirstRowTypedError;
use scylla::{FromRow, Session, SessionBuilder};
use uuid::Uuid;

use super::err_models::VpError;
use super::p_models::UpdatePixel;

//ScyllaBuilder
pub struct ScyllaBuilder {
    session: Session,
}
impl ScyllaBuilder {
    pub async fn try_init(scylla_url: &str) -> Result<Self, VpError> {
        let session = SessionBuilder::new().known_node(scylla_url).build().await?;
        Ok(Self { session })
    }
    async fn init_table(&self) -> Result<(), VpError> {
        self.session.query("CREATE KEYSPACE IF NOT EXISTS vplace WITH REPLICATION = {'class' : 'NetworkTopologyStrategy', 'replication_factor' : 1}", &[]).await?;
        self.session
        .query("CREATE TABLE IF NOT EXISTS vplace.player (id uuid,uname text,x int,y int,color int,last_placed timestamp,primary key (id))", &[])
        .await?;
        Ok(())
    }

    pub async fn try_build(self) -> Result<ScyllaManager, VpError> {
        self.init_table().await?;
        let insert_user=self.session.prepare("INSERT INTO vplace.player (id, uname, x, y, color, last_placed) VALUES (?, ?, ?, ?, ?, ?)").await?;
        let get_user = self
            .session
            .prepare("SELECT id, uname, x, y, color, last_placed FROM vplace.player WHERE id = ?")
            .await?;
        Ok(ScyllaManager {
            session: self.session,
            insert_user,
            get_user,
        })
    }
}

//ScyllaDb Manager
pub struct ScyllaManager {
    session: Session,
    insert_user: PreparedStatement,
    get_user: PreparedStatement,
}
impl ScyllaManager {
    pub async fn get_user(&self, uid: &Uuid) -> Result<UserDetails, VpError> {
        let rows = self.session.execute(&self.get_user, (uid,)).await?;
        let res = rows.first_row_typed::<UserDetails>();
        match res {
            Ok(res) => Ok(res),
            Err(FirstRowTypedError::RowsEmpty) => Err(VpError::NoUserFound),
            Err(e) => Err(VpError::ScyllaTypeErr(e)),
        }
    }
    pub async fn set_user(&self, req: &UpdatePixel) -> Result<(), VpError> {
        let ix = i32::try_from(req.loc.0)?;
        let iy = i32::try_from(req.loc.1)?;
        let ic = i32::try_from(req.color).unwrap();
        self.session
            .execute(
                &self.insert_user,
                (req.uid, &req.uname, ix, iy, ic, Utc::now().timestamp()),
            )
            .await?;
        Ok(())
    }
}

//ScyllaDb RowData
#[derive(FromRow)]
pub struct UserDetails {
    pub id: Uuid,
    pub name: String,
    pub x: i32,     //u32 aan sherikkum , but CQL derive does'nt support : )
    pub y: i32,     // same as above : )
    pub color: i32, // sherikkum u8
    pub last_placed: i64,
}
