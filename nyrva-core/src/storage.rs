//! Private, bounded local state. Never accepts provider credential paths.
use rusqlite::{Connection, OpenFlags};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::Duration};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(crate) fn state_dir(root:&Path)->PathBuf { root.join("telemetry") }
pub(crate) fn check_path(path:&Path)->Result<(),String> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(m) if m.file_type().is_symlink() => return Err("private state must not follow symbolic links".into()),
            Ok(m) => {
                #[cfg(windows)] { use std::os::windows::fs::MetadataExt; if m.file_attributes() & 0x400 != 0 { return Err("private state must not follow reparse points".into()); } }
                #[cfg(not(windows))] let _ = m;
            },
            Err(e) if e.kind()==std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("cannot inspect private state".into()),
        }
    }
    Ok(())
}
pub(crate) fn private_dir(path:&Path)->Result<(),String> {
    check_path(path)?;
    let mut b=fs::DirBuilder::new(); b.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; b.mode(0o700); }
    b.create(path).map_err(|_|"cannot create private state directory".to_string())?;
    if !fs::metadata(path).map_err(|_|"cannot inspect private directory")?.is_dir() { return Err("private state directory is not a directory".into()); }
    Ok(())
}
pub(crate) fn read_optional(path:&Path,limit:usize)->Result<Option<Vec<u8>>,String> {
    check_path(path)?;
    let file=match fs::File::open(path) { Ok(f)=>f,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(_)=>return Err("cannot read private state".into()) };
    if !file.metadata().map_err(|_|"cannot inspect private state")?.is_file() { return Err("private state must be a regular file".into()); }
    let mut data=Vec::new(); file.take((limit+1) as u64).read_to_end(&mut data).map_err(|_|"cannot read private state")?;
    if data.len()>limit { return Err("private state exceeds size limit".into()); } Ok(Some(data))
}
pub(crate) fn atomic_write(path:&Path,data:&[u8])->Result<(),String> {
    check_path(path)?;
    let parent=path.parent().ok_or("private state has no parent")?; private_dir(parent)?;
    if let Ok(m)=fs::symlink_metadata(path) { if !m.is_file() { return Err("private state must be a regular file".into()); } }
    let tmp=parent.join(format!(".nyrva-{}-{}-{}.tmp",std::process::id(),crate::cli::now_ms(),NEXT.fetch_add(1,Ordering::Relaxed)));
    let mut o=fs::OpenOptions::new(); o.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    let result=(|| { let mut f=o.open(&tmp).map_err(|_|"cannot create private transaction file")?; f.write_all(data).and_then(|_|f.sync_all()).map_err(|_|"cannot persist private transaction")?; drop(f); check_path(path)?; fs::rename(&tmp,path).map_err(|_|"cannot commit private state".to_string())?; #[cfg(unix)] { if let Ok(dir)=fs::File::open(parent) { let _=dir.sync_all(); } } Ok(()) })();
    if result.is_err() { let _=fs::remove_file(&tmp); } result
}
/// Serialize control mutations across CLI and desktop. A crashed process releases the lock.
pub(crate) fn control(root:&Path)->Result<Connection,String> {
    let dir=state_dir(root); private_dir(&dir)?; let path=dir.join("control.sqlite3"); check_path(&path)?;
    let mut o=fs::OpenOptions::new(); o.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    match o.open(&path) { Ok(_)=>(),Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>(),Err(_)=>return Err("cannot create control database".into()) }
    if !fs::symlink_metadata(&path).map_err(|_|"cannot inspect control database")?.is_file() { return Err("control database must be a regular file".into()); }
    let db=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|_|"cannot open control database")?;
    db.busy_timeout(Duration::from_secs(5)).map_err(|_|"cannot configure control timeout")?;
    db.execute_batch("PRAGMA secure_delete=ON; BEGIN IMMEDIATE;").map_err(|_|"control database is busy")?;
    let version:i64=db.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|_|"cannot inspect control schema")?;
    if !(0..=1).contains(&version) { return Err("unsupported control database version".into()); }
    if version==0 { db.execute_batch("CREATE TABLE alert_events(id TEXT PRIMARY KEY,at_ms INTEGER NOT NULL,payload TEXT NOT NULL); CREATE INDEX alert_event_time ON alert_events(at_ms); CREATE TABLE alert_windows(scope TEXT PRIMARY KEY,observed INTEGER NOT NULL,reset_at INTEGER,fraction REAL NOT NULL,generation INTEGER NOT NULL); PRAGMA user_version=1;").map_err(|_|"cannot initialize control schema")?; }
    Ok(db)
}
pub(crate) fn commit(db:&Connection)->Result<(),String> { db.execute_batch("COMMIT").map_err(|_|"cannot commit control state".into()) }
