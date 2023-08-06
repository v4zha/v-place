use chrono::Utc;
use scylla::prepared_statement::PreparedStatement;
use scylla::transport::query_result::FirstRowTypedError;
use scylla::{FromRow, IntoUserType, Session, SessionBuilder};
use serde::Serialize;
use uuid::Uuid;

use super::err_models::VpError;
use super::p_models::UpdatePixel;

//ScyllaBuilder
pub struct ScyllaBuilder {
    session: Session,
    dim_mid: u32,
}
impl ScyllaBuilder {
    pub async fn try_init(scylla_url: &str, canvas_dim: u32) -> Result<Self, VpError> {
        let session = SessionBuilder::new().known_node(scylla_url).build().await?;
        let dim_mid = canvas_dim / 2;
        Ok(Self { session, dim_mid })
    }
    async fn init_table(&self) -> Result<(), VpError> {
        //Store Pixel Update of Each User
        //->used to check cooldown
        self.session.query("CREATE KEYSPACE IF NOT EXISTS vplace WITH REPLICATION = {'class' : 'NetworkTopologyStrategy', 'replication_factor' : 1}", &[]).await?;
        //table to store User's last pixel placement
        self.session
        .query("CREATE TABLE IF NOT EXISTS vplace.player (id uuid,uname text,x int,y int,color int,last_placed timestamp,PRIMARY KEY (id))", &[])
        .await?;

        //Store All Pixel data
        // UDT to store pixel_data
        self.session.query("CREATE TYPE IF NOT EXISTS vplace.pixel_data (uname text,last_placed timestamp ,color int)",&[]).await?;
        //table to store all pixel update data in canvas
        // Divide the canvas into 4 parts
        //       ---------------
        //       |      |      |
        //       |   1  |  2   |
        //       |------|------|
        //       |   3  |  4   |
        //       |      |      |
        //       --------------
        // each part is row with pixel details as column of the form (x,y):pixel_data
        // where pixel_data is UDT defined above : ) .
        self.session.query("CREATE TABLE IF NOT EXISTS vplace.canvas ( canvas_part text,pixel map<frozen<tuple<int, int>>, frozen<pixel_data>>,PRIMARY KEY (canvas_part))",&[]).await?;
        Ok(())
    }

    pub async fn try_build(self) -> Result<ScyllaManager, VpError> {
        self.init_table().await?;
        let insert_user=self.session.prepare("INSERT INTO vplace.player (id, uname, x, y, color, last_placed) VALUES (?, ?, ?, ?, ?, ?)").await?;
        let get_user = self
            .session
            .prepare("SELECT id, uname, x, y, color, last_placed FROM vplace.player WHERE id = ?")
            .await?;
        let insert_pixel = self
            .session
            .prepare("INSERT INTO vplace.canvas (canvas_part,pixel) VALUES (?,{(?,?):?})")
            .await?;
        let get_pixel = self
            .session
            .prepare("SELECT pixel FROM vplace.canvas WHERE canvas_part = ? AND pixel CONTAINS KEY (?,?)")
            .await?;
        Ok(ScyllaManager {
            session: self.session,
            dim_mid: self.dim_mid,
            insert_user,
            get_user,
            insert_pixel,
            get_pixel,
            canvas_part: ["v_part1", "v_part2", "v_part3", "v_part4"],
        })
    }
}

//ScyllaDb Manager
pub struct ScyllaManager {
    session: Session,
    dim_mid: u32,
    insert_user: PreparedStatement,
    get_user: PreparedStatement,
    insert_pixel: PreparedStatement,
    get_pixel: PreparedStatement,
    canvas_part: [&'static str; 4],
}
impl ScyllaManager {
    pub async fn get_user(&self, uid: &Uuid) -> Result<UserDetails, VpError> {
        let rows = self.session.execute(&self.get_user, (uid,)).await?;
        let res = rows.first_row_typed::<UserDetails>();
        match res {
            Ok(res) => Ok(res),
            Err(FirstRowTypedError::RowsEmpty) => Err(VpError::InvalidUser),
            Err(e) => Err(VpError::ScyllaTypeErr(e)),
        }
    }
    pub async fn update_db(&self, req: &UpdatePixel) -> Result<(), VpError> {
        let (ix, iy) = (i32::try_from(req.loc.0)?, i32::try_from(req.loc.1)?);
        // infallible :)
        let color = i32::try_from(req.color).unwrap();
        let last_placed = Utc::now().timestamp();
        let uname = req.uname.as_ref().ok_or_else(|| VpError::InvalidUser)?;

        // add user update
        let user_update = self
            .session
            .execute(&self.insert_user, (req.uid, uname, ix, iy, color));

        // add  pixel update
        let pindex = match (req.loc.0 <= self.dim_mid, req.loc.0 <= self.dim_mid) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        };
        let pixel_data = PixelData {
            uname: uname.to_string(),
            color,
            last_placed,
        };
        let pixel_update = self.session.execute(
            &self.insert_pixel,
            (self.canvas_part[pindex], ix, iy, pixel_data),
        );
        tokio::try_join!(user_update, pixel_update)?;
        Ok(())
    }
    pub async fn get_pixel(&self, x: u32, y: u32) -> Result<PixelData, VpError> {
        let ix = i32::try_from(x)?;
        let iy = i32::try_from(y)?;
        let pindex = match (x <= self.dim_mid, y <= self.dim_mid) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        };
        let rows = self
            .session
            .execute(&self.get_pixel, (self.canvas_part[pindex], ix, iy))
            .await?;
        let res = rows.first_row_typed::<PixelData>();
        match res {
            Ok(res) => Ok(res),
            Err(FirstRowTypedError::RowsEmpty) => Err(VpError::NoPixelData),
            Err(e) => Err(VpError::ScyllaTypeErr(e)),
        }
    }
}

//ScyllaDb RowData
#[derive(FromRow)]
pub struct UserDetails {
    pub id: Uuid,
    pub uname: String,
    pub x: i32,     //u32 aan sherikkum , but CQL derive does'nt support : )
    pub y: i32,     // same as above : )
    pub color: i32, // sherikkum u8
    pub last_placed: i64,
}

#[derive(IntoUserType, FromRow, Serialize)]
pub struct PixelData {
    pub uname: String,
    pub color: i32,
    pub last_placed: i64,
}
