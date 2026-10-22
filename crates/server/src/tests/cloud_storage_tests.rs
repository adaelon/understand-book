use super::*;
use base64::Engine;
use crate::control_store::ControlStore;

/// Existing allowance fixtures opt into sold storage rights without introducing
/// a second/overlapping period. Values are test-only, never product defaults.
pub(crate) fn seed(db: &Connection, owner: &str, period: &str, capacity: i64, file_limit: i64) {
    let (start,end):(i64,i64)=db.query_row("SELECT starts_at,expires_at FROM allowance_periods WHERE period_id=?",[period],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let op=format!("storage-{period}");
    db.execute("INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at) VALUES(?,?,?,'storage-test','{}','{}',?)",params![owner,op,owner,start]).unwrap();
    db.execute("INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,channel,note) VALUES(?,?,?,?,1,?,'test','fixture only')",params![op,owner,op,owner,start]).unwrap();
    let product=json!({"product_id":"storage-test","revision":1,"title":"Storage fixture","tier":"basic","duration":"day1","enabled":true,"test_only":true,
        "amount_fen":1,"basis":{"version":"storage-test","basis_micro_cny":100000},"grant_milli_percent":100000,"storage_bytes":capacity,"max_file_bytes":file_limit,"retention_seconds":60,"instructions":""});
    db.execute("INSERT INTO cloud_plan_entitlements(period_id,user_id,receipt_id,product_id,product_revision,tier,duration,starts_at,expires_at,storage_bytes,max_file_bytes,retention_seconds,product_snapshot_json,purchased_at) VALUES(?,?,?,'storage-test',1,'basic','day1',?,?,?,?,60,?,?)",
        params![period,owner,op,start,end,capacity,file_limit,product.to_string(),start]).unwrap();
}

struct Fixture { _root: tempfile::TempDir, access: Arc<Authorization>, control: ControlStore, now:i64 }
impl Fixture {
    fn new(capacity:i64,file_limit:i64)->Self {
        let root=tempfile::tempdir().unwrap();
        let users=crate::user_registry::UserRegistry::open(root.path()).unwrap();
        let mut control=ControlStore::open(users.writer()).unwrap();
        control.create_user("A").unwrap();control.create_user("B").unwrap();
        let now=crate::multi_user_host::now();
        control.connection.execute("INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('p','A',?,?)",params![now-10,now+1000]).unwrap();
        seed(&control.connection,"A","p",capacity,file_limit);
        for w in ["w1","w2"] {
            control.connection.execute("INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES('A',?,?,?)",params![w,now,now]).unwrap();
        }
        Self {access:Arc::new(Authorization::new(users).unwrap()),control,_root:root,now}
    }
    fn upload(&self,w:&str,id:&str,total:usize,offset:usize,bytes:&[u8])->Result<Value,ToolError> {
        crate::personal_preparations::request(&self.access,"A",w,"POST",json!({"action":"upload","upload_id":id,"filename":"notes.md","total":total,"offset":offset,"chunk_base64":base64::engine::general_purpose::STANDARD.encode(bytes)}),self.now)
    }
    fn status(&self)->Value {request(&self.access,"A","GET",Value::Null,self.now).unwrap()}
    fn remove(&self,id:&str,action:&str)->Result<Value,ToolError> {request(&self.access,"A","POST",json!({"action":action,"storage_id":id}),self.now)}
}

#[test]
fn account_upload_reserves_whole_file_across_workspaces_retries_once_and_cancel_releases() {
    let f=Fixture::new(20,15);
    assert_eq!(f.upload("w1","large",16,0,b"x").unwrap_err().error_code,"STORAGE_FILE_TOO_LARGE");
    f.upload("w1","first",12,0,b"abc").unwrap();
    assert_eq!(f.status()["reserved_bytes"],12);
    assert_eq!(f.upload("w2","second",9,0,b"x").unwrap_err().error_code,"STORAGE_LIMIT_EXCEEDED");
    f.upload("w1","first",12,0,b"abc").unwrap();
    assert_eq!(f.status()["total_bytes"],12);
    // A disk interruption may leave only a prefix of the next block.
    let partial=f.control.writer.root().join("personal-preparations/A/w1/uploads/first/source-first.md");
    use std::io::Write;
    fs::OpenOptions::new().append(true).open(partial).unwrap().write_all(b"def").unwrap();
    f.upload("w1","first",12,3,b"defghijkl").unwrap();
    assert_eq!(f.status()["used_bytes"],12);assert_eq!(f.status()["reserved_bytes"],0);
    assert_eq!(f.remove("first","cancel").unwrap()["total_bytes"],0);
    assert_eq!(f.remove("first","cancel").unwrap()["total_bytes"],0);
    assert_eq!(f.upload("w1","first",12,0,b"abc").unwrap_err().error_code,"STORAGE_UPLOAD_CONFLICT");
    f.upload("w2","second",9,0,b"abcdefghi").unwrap();
}

#[test]
fn legacy_originals_are_adopted_without_subscription_and_download_remains_available() {
    let f=Fixture::new(10,10);
    let directory=f.control.writer.root().join("personal-preparations/B/old-workspace/uploads/old-file");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("upload.json"),json!({"filename":"old.md","extension":"md","total":20}).to_string()).unwrap();
    fs::write(directory.join("source.md"),b"01234567890123456789").unwrap();
    let view=request(&f.access,"B","GET",Value::Null,f.now).unwrap();
    assert_eq!(view["used_bytes"],20);assert_eq!(view["can_upload"],false);
    let (path,_)=download(&f.access,"B","old-file").unwrap();
    assert_eq!(fs::read(path).unwrap().len(),20);
    assert_eq!(request(&f.access,"B","POST",json!({"action":"delete","storage_id":"old-file"}),f.now).unwrap()["used_bytes"],0);
}

#[test]
fn removal_and_retention_never_delete_a_publication_still_referenced_by_a_second_workspace() {
    let f=Fixture::new(50,50);
    f.upload("w1","source",5,0,b"hello").unwrap();
    f.control.connection.execute("INSERT INTO book_publications VALUES('book','pub','unneeded-path','{}')",[]).unwrap();
    f.control.connection.execute("INSERT INTO book_grants VALUES('A','book','pub')",[]).unwrap();
    for w in ["w1","w2"] {
        f.control.connection.execute("INSERT INTO learning_workspace_members VALUES('A',?,'book','pub',?)",params![w,f.now]).unwrap();
    }
    f.control.connection.execute("UPDATE cloud_storage_objects SET book_id='book',publication_id='pub' WHERE storage_id='source'",[]).unwrap();
    f.control.connection.execute("DELETE FROM learning_workspace_members WHERE learning_workspace_id='w1'",[]).unwrap();
    assert_eq!(f.remove("source","delete").unwrap_err().error_code,"STORAGE_IN_USE");
    let retained=request(&f.access,"A","POST",json!({"action":"cleanup"}),f.now+2000).unwrap();
    assert_eq!(retained["used_bytes"],5);
    assert!(download(&f.access,"A","source").is_ok());
    // The common curriculum grant itself never consumes private space.
    assert_eq!(retained["items"].as_array().unwrap().len(),1);
}

#[test]
fn legacy_generated_records_do_not_consume_upload_space_or_block_originals() {
    let f=Fixture::new(20,20);
    for state in ["used","reserved"] {
        let path=f.control.writer.root().join(format!("users/A/memory/agent-history.presentations/versions/{state}/1.json"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path,vec![b'x';100]).unwrap();
        f.control.connection.execute("INSERT INTO cloud_storage_objects(owner_user_id,storage_id,kind,filename,path,bytes,state,created_at,updated_at) VALUES('A',?,'generated','1.json',?,100,?,?,?)",
            params![format!("generated:{state}"),path.to_string_lossy(),state,f.now,f.now]).unwrap();
    }
    f.upload("w1","source",12,0,b"abcdefghijkl").unwrap();
    let view=f.status();
    assert_eq!(view["used_bytes"],12);assert_eq!(view["reserved_bytes"],0);
    assert_eq!(view["available_bytes"],8);assert_eq!(view["items"].as_array().unwrap().len(),1);
    assert_eq!(view["items"][0]["storage_id"],"source");
    assert_eq!(f.remove("source","delete").unwrap()["total_bytes"],0);
    cleanup(&f.control.connection,f.control.writer.root(),"A",f.now+2000).unwrap();
    for state in ["used","reserved"] {
        let path=f.control.writer.root().join(format!("users/A/memory/agent-history.presentations/versions/{state}/1.json"));
        assert_eq!(fs::read(path).unwrap(),vec![b'x';100]);
    }
}

#[test]
fn existing_learning_artifact_files_are_not_adopted_as_uploads() {
    let f=Fixture::new(20,20);
    let presentations=f.control.writer.root().join("users/B/memory/agent-history.presentations");
    for directory in ["versions","states","assets"] {
        fs::create_dir_all(presentations.join(directory)).unwrap();
        fs::write(presentations.join(directory).join("retained.json"),b"retained learning artifact").unwrap();
    }
    let view=request(&f.access,"B","GET",Value::Null,f.now).unwrap();
    assert_eq!(view["total_bytes"],0);assert_eq!(view["items"],json!([]));
    let count:i64=f.control.connection.query_row("SELECT count(*) FROM cloud_storage_objects WHERE owner_user_id='B'",[],|r|r.get(0)).unwrap();
    assert_eq!(count,0);
    for directory in ["versions","states","assets"] {
        assert_eq!(fs::read(presentations.join(directory).join("retained.json")).unwrap(),b"retained learning artifact");
    }
    assert_eq!(reserve(&f.control.connection,"B","w","upload","book.pdf",&f.control.writer.root().join("book.pdf"),1,f.now).unwrap_err().error_code,"STORAGE_PLAN_REQUIRED");
}

#[test]
fn expired_entitlement_blocks_new_bytes_but_preserves_exports_until_unreferenced_cleanup() {
    let mut f=Fixture::new(20,20);
    f.upload("w1","source",5,0,b"hello").unwrap();
    f.now+=1001;
    assert_eq!(f.upload("w2","late",1,0,b"x").unwrap_err().error_code,"STORAGE_PLAN_REQUIRED");
    assert_eq!(f.status()["used_bytes"],5);
    assert!(download(&f.access,"A","source").is_ok());
    f.now+=60;
    assert_eq!(f.status()["used_bytes"],0);
    assert!(download(&f.access,"A","source").is_err());
}

#[test]
fn a_lower_next_term_preserves_existing_files_and_blocks_growth() {
    let mut f=Fixture::new(20,20);
    f.upload("w1","source",12,0,b"abcdefghijkl").unwrap();
    f.control.connection.execute("INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('next','A',?,?)",params![f.now+1000,f.now+2000]).unwrap();
    seed(&f.control.connection,"A","next",8,8);
    f.now+=1000;
    assert_eq!(f.status()["over_limit"],true);
    assert!(download(&f.access,"A","source").is_ok());
    assert_eq!(f.upload("w2","extra",1,0,b"x").unwrap_err().error_code,"STORAGE_LIMIT_EXCEEDED");
    assert_eq!(f.remove("source","delete").unwrap()["available_bytes"],8);
}

#[test]
fn maintenance_releases_an_abandoned_upload_without_a_returning_reader() {
    let f=Fixture::new(20,20);
    f.upload("w1","abandoned",12,0,b"abc").unwrap();
    assert_eq!(total(&f.control.connection,"A").unwrap(),(0,12));
    maintenance(&f.access,f.now+UPLOAD_IDLE_SECONDS+1).unwrap();
    assert_eq!(total(&f.control.connection,"A").unwrap(),(0,0));
    let path=f.control.writer.root().join("personal-preparations/A/w1/uploads/abandoned");
    assert!(!path.exists());
}
