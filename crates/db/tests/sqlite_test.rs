use chrono::prelude::*;
use common::crate_data::{CrateData, CrateRegistryDep, CrateVersionData};
use common::crate_overview::CrateOverview;
use common::index_metadata::IndexMetadata;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::prefetch::Prefetch;
use common::publish_metadata::{PublishMetadata, RegistryDep};
use common::util::generate_rand_string;
use common::version::Version;
use db::password::hash_pwd;
use db::provider::PrefetchState;
use db::{ConString, Database, DocQueueEntry, SqliteConString};
use db::{DbProvider, User};
use std::collections::BTreeMap;
use std::ops::Add;
use std::path::PathBuf;
use std::{path, thread, time};

struct TestDB {
    path: PathBuf,
    pub db: Database,
}

impl TestDB {
    async fn new() -> Self {
        let path = path::PathBuf::from("/tmp").join(generate_rand_string(8).add(".db"));

        let con_string = SqliteConString {
            path: path.to_owned(),
            salt: "salt".to_string(),
            admin_pwd: "123".to_string(),
            admin_token: "token".to_string(),
            session_age: time::Duration::from_secs(1),
        };

        let con_string = ConString::Sqlite(con_string);

        let db: Database = Database::new(&con_string).await.unwrap();

        Self { path, db }
    }
}

impl Drop for TestDB {
    fn drop(&mut self) {
        rm_rf::remove(&self.path).expect("Cannot to remove test db");
    }
}

#[tokio::test]
async fn get_total_unique_crates_returns_number_of_unique_crates() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate2",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate3",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();

    let unique_crates = test_db.db.get_total_unique_crates().await.unwrap();

    assert_eq!(3, unique_crates);
}

#[tokio::test]
async fn get_total_crate_versions_returns_number_of_crate_versions() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate2",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();

    let total_versions = test_db.db.get_total_crate_versions().await.unwrap();

    assert_eq!(3, total_versions);
}

#[tokio::test]
async fn get_total_downloads_returns_number_of_total_downloads() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let id1 = test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    let id2 = test_db
        .db
        .test_add_crate(
            "crate2",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_meta(id1, &Version::try_from("1.0.0").unwrap(), &created, None)
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_meta(id1, &Version::try_from("2.0.0").unwrap(), &created, None)
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_meta(id2, &Version::try_from("1.0.0").unwrap(), &created, None)
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("2.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate2"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    let downloads = test_db.db.get_total_downloads().await.unwrap();

    assert_eq!(4, downloads);
}

#[tokio::test]
async fn get_top_crates_downloads_returns_top_crates_with_downloads() {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let test_db = TestDB::new().await;
    let crate_id1 = test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created1,
        )
        .await
        .unwrap();
    let crate_id2 = test_db
        .db
        .test_add_crate(
            "mycrate2",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created1,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_meta(
            crate_id1,
            &Version::try_from("0.1.0").unwrap(),
            &created1,
            None,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_meta(
            crate_id2,
            &Version::try_from("0.1.0").unwrap(),
            &created1,
            None,
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("mycrate"),
            &Version::from_unchecked_str("0.1.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("mycrate"),
            &Version::from_unchecked_str("0.1.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("mycrate2"),
            &Version::from_unchecked_str("0.1.0"),
        )
        .await
        .unwrap();

    let top_crates = test_db.db.get_top_crates_downloads(10).await.unwrap();

    assert_eq!(2, top_crates.len());
    assert_eq!("mycrate", top_crates[0].0);
    assert_eq!(2, top_crates[0].1);
    assert_eq!("mycrate2", top_crates[1].0);
    assert_eq!(1, top_crates[1].1);
}

#[tokio::test]
async fn increase_download_counter_works() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate2",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate3",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate4",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate5",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate1",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate2",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate3",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate4",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate5",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("2.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate2"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate3"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate3"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate5"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate5"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate5"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate5"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    let tops = test_db.db.get_top_crates_downloads(2).await.unwrap();
    assert_eq!(2, tops.len());
    assert_eq!(("crate5".to_string(), 4), tops[0]);
    assert_eq!(("crate1".to_string(), 3), tops[1]);
}

#[tokio::test]
async fn get_max_version_from_id() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    let crate_id1 = test_db
        .db
        .test_add_crate(
            "acrate",
            "admin",
            &Version::try_from("0.1.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "acrate",
            "admin",
            &Version::try_from("0.2.0").unwrap(),
            &created1,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "acrate",
            "admin",
            &Version::try_from("0.10.0").unwrap(),
            &created2,
        )
        .await
        .unwrap();

    let version = test_db.db.get_max_version_from_id(crate_id1).await.unwrap();

    assert_eq!("0.10.0", version.to_string());
}

#[tokio::test]
async fn get_crate_summaries_works() {
    let test_db = TestDB::new().await;
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 11, 22, 12).unwrap();
    test_db
        .db
        .test_add_crate(
            "acrate",
            "admin",
            &Version::try_from("1.1.0").unwrap(),
            &created1,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "bcrate",
            "admin",
            &Version::try_from("1.1.0").unwrap(),
            &created2,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "acrate",
            "admin",
            &Version::try_from("1.2.0").unwrap(),
            &created2,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "bcrate",
            "admin",
            &Version::try_from("1.1.0").unwrap(),
            &created1,
        )
        .await
        .unwrap();

    let crates = test_db.db.get_crate_summaries().await.unwrap();

    assert_eq!(2, crates.len());
    assert_eq!("acrate", crates[0].name);
    assert_eq!("1.2.0", crates[0].max_version);
    assert_eq!(0, crates[0].total_downloads);
    assert_eq!("2020-10-08 11:22:12", crates[0].last_updated);

    assert_eq!("bcrate", crates[1].name);
    assert_eq!("1.1.0", crates[1].max_version);
    assert_eq!(0, crates[1].total_downloads);
    assert_eq!("2020-10-07 13:18:00", crates[1].last_updated);
}

#[tokio::test]
async fn is_owner_true() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();

    assert!(test_db
        .db
        .is_owner(
            &NormalizedName::from_unchecked("mycrate".to_string()),
            "admin"
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn is_owner_false() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();

    assert!(!test_db
        .db
        .is_owner(
            &NormalizedName::from_unchecked("mycrate".to_string()),
            "user"
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn delete_owner_valid_owner() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();

    test_db.db.delete_owner("mycrate", "admin").await.unwrap();

    assert!(test_db
        .db
        .get_crate_owners(&NormalizedName::from_unchecked("mycrate".to_string()))
        .await
        .is_ok());
}

#[tokio::test]
async fn add_crate_if_not_exists_duplicate() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();

    let owners = test_db
        .db
        .get_crate_owners(&NormalizedName::from_unchecked("mycrate".to_string()))
        .await
        .unwrap();
    assert_eq!(1, owners.len());
    assert_eq!("admin", owners[0].name);
}

#[tokio::test]
async fn add_crate_different_user() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_user("user", "123", "123", false)
        .await
        .unwrap();

    test_db
        .db
        .test_add_crate(
            "mycrate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "mycrate",
            "user",
            &Version::try_from("1.0.0").unwrap(),
            &Utc::now(),
        )
        .await
        .unwrap();

    let owners = test_db
        .db
        .get_crate_owners(&NormalizedName::from_unchecked("mycrate".to_string()))
        .await
        .unwrap();
    assert_eq!(2, owners.len());
    assert_eq!("admin", owners[0].name);
    assert_eq!("user", owners[1].name);
}

#[tokio::test]
async fn get_user_from_token_works() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_auth_token("test1", "mytoken1", "admin")
        .await
        .unwrap();

    let user = test_db.db.get_user_from_token("mytoken1").await.unwrap();

    assert_eq!("admin", user.name);
}

#[tokio::test]
async fn get_auth_tokens_returns_all_tokens() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_auth_token("test1", "mytoken1", "admin")
        .await
        .unwrap();
    test_db
        .db
        .add_auth_token("test2", "mytoken2", "admin")
        .await
        .unwrap();

    let tokens = test_db.db.get_auth_tokens("admin").await.unwrap();

    assert_eq!(3, tokens.len());
    assert_eq!("admin", tokens[0].name);
    assert_eq!("test1", tokens[1].name);
    assert_eq!("test2", tokens[2].name);
}

#[tokio::test]
async fn auth_token_insert_and_read() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();
    let user = test_db.db.get_user_from_token("mytoken").await.unwrap();

    assert_eq!("admin", user.name);
}

#[tokio::test]
async fn auth_token_insert_and_delete() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();

    test_db.db.delete_auth_token(2).await.unwrap();

    assert!(test_db.db.get_user_from_token("mytoken").await.is_err());
}

#[tokio::test]
async fn get_user_from_token_no_token() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();

    assert!(test_db.db.get_user_from_token("wrong_token").await.is_err());
}

#[tokio::test]
async fn add_auth_token_no_user() {
    let test_db = TestDB::new().await;

    assert!(test_db
        .db
        .add_auth_token("test", "mytoken", "nouser")
        .await
        .is_err());
}

#[tokio::test]
async fn delete_user_with_sessions() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_user("user", "pwd", "salt", false)
        .await
        .unwrap();
    test_db.db.add_session_token("user", "123").await.unwrap();
    test_db.db.add_session_token("user", "abc").await.unwrap();

    test_db.db.delete_user("user").await.unwrap();

    assert!(test_db.db.validate_session("123").await.is_err());
    assert!(test_db.db.validate_session("abc").await.is_err());
    assert!(test_db.db.get_user("user").await.is_err());
}

#[tokio::test]
async fn add_user_works() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_user("user", "pwd", "salt", false)
        .await
        .unwrap();

    let expected = User {
        id: 2,
        name: "user".to_owned(),
        pwd: hash_pwd("pwd", "salt"),
        salt: "salt".to_owned(),
        is_admin: false,
    };
    let user = test_db.db.get_user("user").await.unwrap();
    assert_eq!(expected, user);
}

#[tokio::test]
async fn add_user_duplicate() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_user("user", "pwd", "salt", false)
        .await
        .unwrap();

    assert!(test_db
        .db
        .add_user("user", "pwd", "salt", false)
        .await
        .is_err())
}

#[tokio::test]
async fn get_users_works() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_user("user", "123", "abc", false)
        .await
        .unwrap();

    let users = test_db.db.get_users().await.unwrap();

    assert_eq!(2, users.len());
    assert_eq!("admin", users[0].name);
    assert_eq!("user", users[1].name);
}

#[tokio::test]
async fn get_user_existing_user() {
    let test_db = TestDB::new().await;
    let users = test_db.db.get_user("admin").await.unwrap();

    assert_eq!("admin", users.name);
}

#[tokio::test]
async fn get_user_no_user() {
    let test_db = TestDB::new().await;

    assert!(test_db.db.get_user("no_user").await.is_err());
}

#[tokio::test]
async fn change_pwd_works() {
    let test_db = TestDB::new().await;

    test_db.db.change_pwd("admin", "abc").await.unwrap();

    assert!(test_db.db.authenticate_user("admin", "abc").await.is_ok());
}

#[tokio::test]
async fn clean_db_after_time() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let (name, _) = test_db.db.validate_session("session_token").await.unwrap();
    assert_eq!("admin", name);

    let duration = time::Duration::from_secs(2);
    thread::sleep(duration);
    test_db
        .db
        .clean_db(time::Duration::from_secs(1))
        .await
        .unwrap();

    assert!(test_db.db.validate_session("session_token").await.is_err());
}

#[tokio::test]
async fn delete_session_token_works() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let (name, _) = test_db.db.validate_session("session_token").await.unwrap();
    assert_eq!("admin", name);

    test_db
        .db
        .delete_session_token("session_token")
        .await
        .unwrap();

    let r = test_db.db.validate_session("session_token");
    assert!(r.await.is_err());
}

#[tokio::test]
async fn delete_session_token_no_token() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let (name, _) = test_db.db.validate_session("session_token").await.unwrap();
    assert_eq!("admin", name);

    let r = test_db.db.delete_session_token("no_token").await;

    assert!(r.is_ok());
}

#[tokio::test]
async fn get_name_valid_user_and_token() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let (name, _) = test_db.db.validate_session("session_token").await.unwrap();

    assert_eq!("admin", name);
}

#[tokio::test]
async fn get_session_no_session_in_db() {
    let test_db = TestDB::new().await;

    assert!(test_db
        .db
        .validate_session("no_session_token")
        .await
        .is_err());
}

#[tokio::test]
async fn bootstrap_db_inserts_admin() {
    let test_db = TestDB::new().await;

    let admin = test_db.db.get_user("admin").await.unwrap();
    assert_eq!(1, admin.id);
    assert_eq!("admin", admin.name);
    assert_eq!(
        "81d40d94fee4fb4eeb1a21bb7adb93c06aad35b929c1a2b024ae33b3a9b79e23",
        admin.pwd
    );
    assert_eq!("salt", admin.salt);
    assert!(admin.is_admin);
}

#[tokio::test]
async fn authenticate_user_valid() {
    let test_db = TestDB::new().await;

    assert!(test_db.db.authenticate_user("admin", "123").await.is_ok());
}

#[tokio::test]
async fn authenticate_user_unknown_user() {
    let test_db = TestDB::new().await;

    assert!(test_db
        .db
        .authenticate_user("unknown", "123")
        .await
        .is_err());
}

#[tokio::test]
async fn authenticate_user_wrong_pwd() {
    let test_db = TestDB::new().await;

    assert!(test_db.db.authenticate_user("admin", "abc").await.is_err());
}

#[tokio::test]
async fn add_and_get_doc_queue_entries() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_doc_queue(
            &NormalizedName::from_unchecked("mycrate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            &PathBuf::from("/tmp/foo"),
        )
        .await
        .unwrap();
    test_db
        .db
        .add_doc_queue(
            &NormalizedName::from_unchecked("mycrate2".to_string()),
            &Version::try_from("2.0.0").unwrap(),
            &PathBuf::from("/tmp/bar"),
        )
        .await
        .unwrap();

    let queue_entries = test_db.db.get_doc_queue().await.unwrap();

    assert_eq!(
        DocQueueEntry {
            id: 1,
            krate: NormalizedName::from_unchecked("mycrate".to_string()),
            version: "1.0.0".to_string(),
            path: PathBuf::from("/tmp/foo")
        },
        queue_entries[0]
    );

    assert_eq!(
        DocQueueEntry {
            id: 2,
            krate: NormalizedName::from_unchecked("mycrate2".to_string()),
            version: "2.0.0".to_string(),
            path: PathBuf::from("/tmp/bar")
        },
        queue_entries[1]
    );
}

#[tokio::test]
async fn delete_doc_queue_entry() {
    let test_db = TestDB::new().await;

    test_db
        .db
        .add_doc_queue(
            &NormalizedName::from_unchecked("mycrate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            &PathBuf::from("/tmp/foo"),
        )
        .await
        .unwrap();
    test_db
        .db
        .add_doc_queue(
            &NormalizedName::from_unchecked("mycrate2".to_string()),
            &Version::try_from("2.0.0").unwrap(),
            &PathBuf::from("/tmp/bar"),
        )
        .await
        .unwrap();

    let queue_entries = test_db.db.get_doc_queue().await.unwrap();
    assert_eq!(2, queue_entries.len());
    test_db.db.delete_doc_queue(1).await.unwrap();

    let queue_entries = test_db.db.get_doc_queue().await.unwrap();
    assert_eq!(1, queue_entries.len());
    assert_eq!(
        DocQueueEntry {
            id: 2,
            krate: NormalizedName::from_unchecked("mycrate2".to_string()),
            version: "2.0.0".to_string(),
            path: PathBuf::from("/tmp/bar")
        },
        queue_entries[0]
    );
}

#[tokio::test]
async fn delete_crate_one_of_multiple_versions() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("3.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();

    let total_versions_before = test_db.db.get_total_crate_versions().await.unwrap();

    test_db
        .db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::try_from("2.0.0").unwrap(),
        )
        .await
        .unwrap();

    let summaries = test_db.db.get_crate_meta_list(1).await.unwrap();
    let krate = test_db
        .db
        .get_crate_id(&NormalizedName::from_unchecked("crate".to_string()))
        .await
        .unwrap();
    assert_eq!(3, total_versions_before);
    assert_eq!(2, summaries.len());
    assert_eq!("1.0.0", summaries[0].version);
    assert_eq!("3.0.0", summaries[1].version);
    assert_eq!(1, krate.unwrap());
}

#[tokio::test]
async fn delete_crate_max_version() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("3.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    let total_versions_before = test_db.db.get_total_crate_versions().await.unwrap();

    test_db
        .db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::try_from("3.0.0").unwrap(),
        )
        .await
        .unwrap();

    let crate_metas = test_db.db.get_crate_meta_list(1).await.unwrap();
    let krate = test_db
        .db
        .get_crate_id(&NormalizedName::from_unchecked("crate".to_string()))
        .await
        .unwrap()
        .unwrap();
    let max_version = test_db.db.get_max_version_from_id(krate).await.unwrap();
    assert_eq!(3, total_versions_before);
    assert_eq!(2, crate_metas.len());
    assert_eq!("1.0.0", crate_metas[0].version);
    assert_eq!("2.0.0", crate_metas[1].version);
    assert_eq!("2.0.0", max_version.to_string());
    assert_eq!(1, krate);
}

#[tokio::test]
async fn delete_crate_only_versions() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    let total_versions_before = test_db.db.get_total_crate_versions().await.unwrap();

    test_db
        .db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
        )
        .await
        .unwrap();

    let summaries = test_db.db.get_crate_meta_list(1).await.unwrap();
    let krate = test_db
        .db
        .get_crate_id(&NormalizedName::from_unchecked("crate".to_string()))
        .await
        .unwrap();
    assert_eq!(1, total_versions_before);
    assert_eq!(0, summaries.len());
    assert!(krate.is_none());
}

#[tokio::test]
async fn search_in_crate_name_found_match() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created_string = created.format("%Y-%m-%d %H:%M:%S").to_string();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(4),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate",
            "admin",
            &Version::try_from("2.2.0").unwrap(),
            &created,
            Some(4),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "foo_crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(3),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "foo_crate",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
            Some(3),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate_foo",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(5),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate_foo",
            "admin",
            &Version::try_from("3.0.0").unwrap(),
            &created,
            Some(5),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "no_match",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(1),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "no_match",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
            Some(1),
        )
        .await
        .unwrap();
    let expected = vec![
        CrateOverview {
            original_name: "crate".to_string(),
            max_version: "2.2.0".to_string(),
            last_updated: created_string.clone(),
            total_downloads: 8,
            ..CrateOverview::default()
        },
        CrateOverview {
            original_name: "crate_foo".to_string(),
            max_version: "3.0.0".to_string(),
            last_updated: created_string.clone(),
            total_downloads: 10,
            ..CrateOverview::default()
        },
        CrateOverview {
            original_name: "foo_crate".to_string(),
            max_version: "2.0.0".to_string(),
            last_updated: created_string.clone(),
            total_downloads: 6,
            ..CrateOverview::default()
        },
    ];

    let search_results = test_db.db.search_in_crate_name("crate").await.unwrap();

    assert_eq!(expected, search_results);
}

#[tokio::test]
async fn get_crate_overview_list() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created_string = created.format("%Y-%m-%d %H:%M:%S").to_string();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(4),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate",
            "admin",
            &Version::try_from("2.2.0").unwrap(),
            &created,
            Some(4),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "foo_crate",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(3),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "foo_crate",
            "admin",
            &Version::try_from("2.0.0").unwrap(),
            &created,
            Some(3),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate_foo",
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
            Some(5),
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate_with_downloads(
            "crate_foo",
            "admin",
            &Version::try_from("3.0.0").unwrap(),
            &created,
            Some(5),
        )
        .await
        .unwrap();
    let expected = vec![
        CrateOverview {
            original_name: "crate".to_string(),
            max_version: "2.2.0".to_string(),
            last_updated: created_string.clone(),
            total_downloads: 8,
            ..CrateOverview::default()
        },
        CrateOverview {
            original_name: "crate_foo".to_string(),
            max_version: "3.0.0".to_string(),
            last_updated: created_string.clone(),
            total_downloads: 10,
            ..CrateOverview::default()
        },
        CrateOverview {
            original_name: "foo_crate".to_string(),
            max_version: "2.0.0".to_string(),
            last_updated: created_string.clone(),
            total_downloads: 6,
            ..CrateOverview::default()
        },
    ];

    let overview_list = test_db.db.get_crate_overview_list().await.unwrap();

    assert_eq!(expected, overview_list);
}

#[tokio::test]
async fn add_crate_and_get_crate_data() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created_string = created.format("%Y-%m-%d %H:%M:%S").to_string();
    let pm1_v1 = PublishMetadata {
        name: "crate1".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![
            RegistryDep {
                name: "dep1".to_string(),
                version_req: "5.0.0".to_string(),
                features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
                optional: false,
                default_features: true,
                target: Some("normal".to_string()),
                kind: Some("kind1".to_string()),
                registry: Some("registry1".to_string()),
                explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
            },
            RegistryDep {
                name: "dep2".to_string(),
                version_req: "6.0.0".to_string(),
                features: Some(vec!["feature3".to_string(), "feature4".to_string()]),
                optional: true,
                default_features: false,
                target: Some("normal".to_string()),
                kind: Some("kind2".to_string()),
                registry: Some("registry2".to_string()),
                explicit_name_in_toml: None,
            },
        ],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        documentation: Some("documentation1".to_string()),
        homepage: Some("homepage1".to_string()),
        readme: Some("readme1".to_string()),
        readme_file: Some("readme_file1".to_string()),
        keywords: vec!["keyword1".to_string(), "keyword2".to_string()],
        categories: vec!["category1".to_string(), "category2".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        repository: Some("repository1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
    };
    let pm1_v2 = PublishMetadata {
        name: "crate1".to_string(),
        vers: "2.0.0".to_string(),
        deps: vec![
            RegistryDep {
                name: "dep1".to_string(),
                version_req: "5.0.0".to_string(),
                features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
                optional: false,
                default_features: true,
                target: Some("normal".to_string()),
                kind: Some("kind1".to_string()),
                registry: Some("registry1".to_string()),
                explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
            },
            RegistryDep {
                name: "dep2".to_string(),
                version_req: "6.0.0".to_string(),
                features: Some(vec!["feature3".to_string(), "feature4".to_string()]),
                optional: true,
                default_features: false,
                target: Some("normal".to_string()),
                kind: Some("kind2".to_string()),
                registry: Some("registry2".to_string()),
                explicit_name_in_toml: None,
            },
        ],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec![
            "author1".to_string(),
            "author2".to_string(),
            "author3".to_string(),
        ]),
        description: Some("description2".to_string()),
        documentation: Some("documentation2".to_string()),
        homepage: Some("homepage2".to_string()),
        readme: Some("readme2".to_string()),
        readme_file: Some("readme_file2".to_string()),
        keywords: vec!["keyword1".to_string()],
        categories: vec!["category1".to_string(), "category4".to_string()],
        license: Some("license2".to_string()),
        license_file: Some("license_file2".to_string()),
        repository: Some("repository2".to_string()),
        badges: None,
        links: Some("links2".to_string()),
    };
    let pm2_v1 = PublishMetadata {
        name: "crate2".to_string(),
        vers: "3.0.0".to_string(),
        deps: vec![
            RegistryDep {
                name: "dep1".to_string(),
                version_req: "5.0.0".to_string(),
                features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
                optional: false,
                default_features: true,
                target: Some("normal".to_string()),
                kind: Some("kind1".to_string()),
                registry: Some("registry1".to_string()),
                explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
            },
            RegistryDep {
                name: "dep2".to_string(),
                version_req: "6.0.0".to_string(),
                features: Some(vec!["feature3".to_string(), "feature4".to_string()]),
                optional: true,
                default_features: false,
                target: Some("normal".to_string()),
                kind: Some("kind2".to_string()),
                registry: Some("registry2".to_string()),
                explicit_name_in_toml: None,
            },
        ],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        documentation: Some("documentation1".to_string()),
        homepage: Some("homepage1".to_string()),
        readme: Some("readme1".to_string()),
        readme_file: Some("readme_file1".to_string()),
        keywords: vec!["keyword1".to_string(), "keyword2".to_string()],
        categories: vec!["category1".to_string(), "category2".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        repository: Some("repository1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
    };
    let pm2_v2 = PublishMetadata {
        name: "crate2".to_string(),
        vers: "4.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep1".to_string(),
            version_req: "5.0.0".to_string(),
            features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
            optional: false,
            default_features: true,
            target: Some("normal".to_string()),
            kind: Some("kind1".to_string()),
            registry: Some("registry1".to_string()),
            explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
        }],
        ..Default::default()
    };
    test_db
        .db
        .add_user("owner1", "pwd1", "salt1", false)
        .await
        .unwrap();
    test_db
        .db
        .add_user("owner2", "pwd2", "salt2", false)
        .await
        .unwrap();

    // Test, if adding a new crate with multiple versions works as expected
    test_db
        .db
        .add_crate(&pm1_v1, "cksum1_1", &created, "owner1")
        .await
        .unwrap();
    test_db
        .db
        .add_owner(
            &NormalizedName::from_unchecked("crate1".to_string()),
            "owner2",
        )
        .await
        .unwrap();

    let crate_data1_v1 = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    let etag1_v1 = test_db.db.get_prefetch_data("crate1").await.unwrap().etag;

    assert_eq!(
        CrateData {
            name: pm1_v1.name.clone(),
            owners: vec!["owner1".to_string(), "owner2".to_string()],
            max_version: pm1_v1.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm1_v1.homepage.clone(),
            description: pm1_v1.description.clone(),
            repository: pm1_v1.repository.clone(),
            categories: pm1_v1.categories.clone(),
            keywords: pm1_v1.keywords.clone(),
            authors: pm1_v1.authors.clone().unwrap(),
            versions: vec![CrateVersionData {
                version: pm1_v1.vers.clone(),
                created: created_string.clone(),
                downloads: 0,
                readme: pm1_v1.readme.clone(),
                license: pm1_v1.license.clone(),
                license_file: pm1_v1.license_file.clone(),
                documentation: pm1_v1.documentation.clone(),
                dependencies: pm1_v1
                    .deps
                    .clone()
                    .into_iter()
                    .map(CrateRegistryDep::from)
                    .collect(),
                checksum: "cksum1_1".to_string(),
                features: pm1_v1.features.clone(),
                yanked: false,
                links: pm1_v1.links.clone(),
                v: 1,
            }],
        },
        crate_data1_v1
    );

    test_db
        .db
        .add_crate(&pm1_v2, "cksum1_2", &created, "owner1")
        .await
        .unwrap();

    let crate_data1_v2 = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    let etag1_v2 = test_db.db.get_prefetch_data("crate1").await.unwrap().etag;

    assert_eq!(
        CrateData {
            name: pm1_v2.name.clone(),
            owners: vec!["owner1".to_string(), "owner2".to_string()],
            max_version: pm1_v2.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm1_v2.homepage.clone(),
            description: pm1_v2.description.clone(),
            categories: pm1_v2.categories.clone(),
            repository: pm1_v2.repository.clone(),
            keywords: pm1_v2.keywords.clone(),
            authors: pm1_v2.authors.clone().unwrap(),
            versions: vec![
                CrateVersionData {
                    version: pm1_v2.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    readme: pm1_v2.readme.clone(),
                    license: pm1_v2.license.clone(),
                    license_file: pm1_v2.license_file.clone(),
                    documentation: pm1_v2.documentation.clone(),
                    dependencies: pm1_v2
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum1_2".to_string(),
                    features: pm1_v2.features.clone(),
                    yanked: false,
                    links: pm1_v2.links.clone(),
                    v: 1,
                },
                CrateVersionData {
                    version: pm1_v1.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    readme: pm1_v1.readme.clone(),
                    license: pm1_v1.license.clone(),
                    license_file: pm1_v1.license_file.clone(),
                    documentation: pm1_v1.documentation.clone(),
                    dependencies: pm1_v1
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum1_1".to_string(),
                    features: pm1_v1.features.clone(),
                    yanked: false,
                    links: pm1_v1.links.clone(),
                    v: 1,
                },
            ],
        },
        crate_data1_v2
    );

    // Check that the etag changed with newly published version
    assert_ne!(etag1_v1, etag1_v2);

    test_db
        .db
        .add_crate(&pm2_v1, "cksum2_1", &created, "owner2")
        .await
        .unwrap();

    let crate_data2_v1 = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked("crate2".to_string()))
        .await
        .unwrap();

    assert_eq!(
        CrateData {
            name: pm2_v1.name.clone(),
            owners: vec!["owner2".to_string()],
            max_version: pm2_v1.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm2_v1.homepage.clone(),
            description: pm2_v1.description.clone(),
            categories: pm2_v1.categories.clone(),
            keywords: pm2_v1.keywords.clone(),
            authors: pm2_v1.authors.clone().unwrap(),
            repository: pm2_v1.repository.clone(),
            versions: vec![CrateVersionData {
                version: pm2_v1.vers.clone(),
                created: created_string.clone(),
                downloads: 0,
                documentation: pm2_v1.documentation.clone(),
                readme: pm2_v1.readme.clone(),
                license: pm2_v1.license.clone(),
                license_file: pm2_v1.license_file.clone(),
                dependencies: pm2_v1
                    .deps
                    .clone()
                    .into_iter()
                    .map(CrateRegistryDep::from)
                    .collect(),
                checksum: "cksum2_1".to_string(),
                features: pm2_v1.features.clone(),
                yanked: false,
                links: pm2_v1.links.clone(),
                v: 1,
            }],
        },
        crate_data2_v1
    );

    test_db
        .db
        .add_crate(&pm2_v2, "cksum2_2", &created, "owner2")
        .await
        .unwrap();

    let crate_data2_v2 = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked("crate2".to_string()))
        .await
        .unwrap();

    assert_eq!(
        CrateData {
            name: pm2_v2.name.clone(),
            owners: vec!["owner2".to_string()],
            max_version: pm2_v2.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm2_v2.homepage.clone(),
            description: pm2_v2.description.clone(),
            repository: pm2_v2.repository.clone(),
            categories: Vec::default(),
            keywords: Vec::default(),
            authors: Vec::default(),
            versions: vec![
                CrateVersionData {
                    version: pm2_v2.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    documentation: pm2_v2.documentation.clone(),
                    readme: pm2_v2.readme.clone(),
                    license: pm2_v2.license.clone(),
                    license_file: pm2_v2.license_file.clone(),
                    dependencies: pm2_v2
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum2_2".to_string(),
                    features: Default::default(),
                    yanked: false,
                    links: pm2_v2.links.clone(),
                    v: 1,
                },
                CrateVersionData {
                    version: pm2_v1.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    documentation: pm2_v1.documentation.clone(),
                    dependencies: pm2_v1
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    readme: pm2_v1.readme.clone(),
                    license: pm2_v1.license.clone(),
                    license_file: pm2_v1.license_file.clone(),
                    checksum: "cksum2_1".to_string(),
                    features: pm2_v1.features.clone(),
                    yanked: false,
                    links: pm2_v1.links.clone(),
                    v: 1,
                }
            ],
        },
        crate_data2_v2
    );
}

#[tokio::test]
async fn update_docs_link() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let pm = PublishMetadata::minimal("crate1", "1.0.0");
    test_db
        .db
        .add_user("owner1", "pwd", "salt", false)
        .await
        .unwrap();
    test_db
        .db
        .add_crate(&pm, "cksum1_1", &created, "owner1")
        .await
        .unwrap();

    let crate_before = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    assert_eq!(crate_before.versions[0].documentation, None);

    test_db
        .db
        .update_docs_link(
            &NormalizedName::from_unchecked("crate1".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "https://docs.rs/crate1/1.0.0/crate1/",
        )
        .await
        .unwrap();

    let crate_after = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    assert_eq!(
        crate_after.versions[0].documentation,
        Some("https://docs.rs/crate1/1.0.0/crate1/".to_string())
    );
}

#[tokio::test]
async fn get_prefetch_data_with_minimal_data() {
    let test_db = TestDB::new().await;
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::from_unchecked_str("1.0.0"),
            &created1,
        )
        .await
        .unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::from_unchecked_str("2.0.0"),
            &created2,
        )
        .await
        .unwrap();

    let prefetch_data = test_db.db.get_prefetch_data("crate").await.unwrap();

    assert_eq!(
        "8723f3d52d131ea686ea8e517c7f1deac5585fdcc19186f373f88a263119f83b",
        prefetch_data.etag
    );
    assert_eq!(
        created2.format("%Y-%m-%d %H:%M:%S").to_string(),
        prefetch_data.last_modified
    );
    assert_eq!(185, prefetch_data.data.len());
}

#[tokio::test]
async fn get_prefetch_data_with_full_data() {
    let test_db = TestDB::new().await;
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    let created3 = Utc.with_ymd_and_hms(2020, 10, 9, 13, 18, 00).unwrap();
    let pm1 = PublishMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep1".to_string(),
            version_req: "1.0.0".to_string(),
            features: Some(vec!["feat1".to_string(), "feat2".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind1".to_string()),
            registry: Some("registry1".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        homepage: Some("homepage1".to_string()),
        documentation: Some("documentation1".to_string()),
        readme: Some("readme1".to_string()),
        repository: Some("repository1".to_string()),
        keywords: vec!["keyword1".to_string()],
        categories: vec!["category1".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
        readme_file: Some("readme_file1".to_string()),
    };
    let pm2 = PublishMetadata {
        name: "crate".to_string(),
        vers: "2.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep2".to_string(),
            version_req: "2.0.0".to_string(),
            features: Some(vec!["feat2".to_string(), "feat3".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind2".to_string()),
            registry: Some("registry2".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature3".to_string(), vec!["dep2".to_string()]),
            ("feature4".to_string(), vec!["dep2".to_string()]),
        ]),
        authors: Some(vec!["author2".to_string(), "author3".to_string()]),
        description: Some("description2".to_string()),
        homepage: Some("homepage2".to_string()),
        documentation: Some("documentation2".to_string()),
        readme: Some("readme2".to_string()),
        repository: Some("repository2".to_string()),
        keywords: vec!["keyword2".to_string()],
        categories: vec!["category2".to_string()],
        license: Some("license2".to_string()),
        license_file: Some("license_file2".to_string()),
        badges: None,
        links: Some("links2".to_string()),
        readme_file: Some("readme_file2".to_string()),
    };
    let pm3 = PublishMetadata {
        name: "crate".to_string(),
        vers: "3.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep3".to_string(),
            version_req: "3.0.0".to_string(),
            features: Some(vec!["feat3".to_string(), "feat4".to_string()]),
            optional: true,
            default_features: false,
            target: Some("target3".to_string()),
            kind: Some("kind3".to_string()),
            registry: Some("registry3".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature5".to_string(), vec!["dep3".to_string()]),
            ("feature6".to_string(), vec!["dep3".to_string()]),
        ]),
        authors: Some(vec!["author3".to_string(), "author4".to_string()]),
        description: Some("description3".to_string()),
        homepage: Some("homepage3".to_string()),
        documentation: Some("documentation3".to_string()),
        readme: Some("readme3".to_string()),
        repository: Some("repository3".to_string()),
        keywords: vec!["keyword3".to_string()],
        categories: vec!["category3".to_string()],
        license: Some("license3".to_string()),
        license_file: Some("license_file3".to_string()),
        badges: None,
        links: Some("links3".to_string()),
        readme_file: Some("readme_file3".to_string()),
    };

    test_db
        .db
        .add_crate(&pm1, "cksum1_1", &created1, "admin")
        .await
        .unwrap();
    test_db
        .db
        .add_crate(&pm2, "cksum2_1", &created2, "admin")
        .await
        .unwrap();
    test_db
        .db
        .add_crate(&pm3, "cksum3_1", &created3, "admin")
        .await
        .unwrap();

    let prefetch_data = test_db.db.get_prefetch_data("crate").await.unwrap();

    assert_eq!(921, prefetch_data.data.len());
    assert_eq!(
        created3.format("%Y-%m-%d %H:%M:%S").to_string(),
        prefetch_data.last_modified
    );
    assert_eq!(
        "bd18a71d56aff9f39c05a9c819d0363b6d6e917f961a49c126ade402667d8568",
        prefetch_data.etag
    );
}

#[tokio::test]
async fn delete_updates_etag() {
    let test_db = TestDB::new().await;
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    let pm1 = PublishMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep1".to_string(),
            version_req: "1.0.0".to_string(),
            features: Some(vec!["feat1".to_string(), "feat2".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind1".to_string()),
            registry: Some("registry1".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        homepage: Some("homepage1".to_string()),
        documentation: Some("documentation1".to_string()),
        readme: Some("readme1".to_string()),
        repository: Some("repository1".to_string()),
        keywords: vec!["keyword1".to_string()],
        categories: vec!["category1".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
        readme_file: Some("readme_file1".to_string()),
    };
    let pm2 = PublishMetadata {
        name: "crate".to_string(),
        vers: "2.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep2".to_string(),
            version_req: "2.0.0".to_string(),
            features: Some(vec!["feat2".to_string(), "feat3".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind2".to_string()),
            registry: Some("registry2".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature3".to_string(), vec!["dep2".to_string()]),
            ("feature4".to_string(), vec!["dep2".to_string()]),
        ]),
        authors: Some(vec!["author2".to_string(), "author3".to_string()]),
        description: Some("description2".to_string()),
        homepage: Some("homepage2".to_string()),
        documentation: Some("documentation2".to_string()),
        readme: Some("readme2".to_string()),
        repository: Some("repository2".to_string()),
        keywords: vec!["keyword2".to_string()],
        categories: vec!["category2".to_string()],
        license: Some("license2".to_string()),
        license_file: Some("license_file2".to_string()),
        badges: None,
        links: Some("links2".to_string()),
        readme_file: Some("readme_file2".to_string()),
    };
    test_db
        .db
        .add_crate(&pm1, "cksum1_1", &created1, "admin")
        .await
        .unwrap();
    test_db
        .db
        .add_crate(&pm2, "cksum2_1", &created2, "admin")
        .await
        .unwrap();
    let prefetch_data = test_db.db.get_prefetch_data("crate").await.unwrap();
    let etag_before = prefetch_data.etag;

    test_db
        .db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    let prefetch_data = test_db.db.get_prefetch_data("crate").await.unwrap();
    let etag_after = prefetch_data.etag;

    assert_ne!(etag_before, etag_after);
}

#[tokio::test]
async fn is_cratesio_cache_up_to_date_not_found() {
    let test_db = TestDB::new().await;

    let prefetch_state = test_db
        .db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("last_modified".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(PrefetchState::NotFound, prefetch_state);
}

#[tokio::test]
async fn is_cratesio_cache_up_to_date_up_to_date() {
    let test_db = TestDB::new().await;
    test_db
        .db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked_str("crate".to_string()),
            "etag",
            "last_modified",
            None,
            &[],
        )
        .await
        .unwrap();

    let prefetch_state = test_db
        .db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("last_modified".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(PrefetchState::UpToDate, prefetch_state);
}

#[tokio::test]
async fn is_cratesio_cache_up_to_date_needs_update() {
    let test_db = TestDB::new().await;
    let indices1 = vec![IndexMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![],
        cksum: "cksum".to_string(),
        features: Default::default(),
        yanked: false,
        links: None,
        v: Some(1),
        features2: None,
    }];
    test_db
        .db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked_str("crate".to_string()),
            "etag",
            "last_modified",
            None,
            &indices1,
        )
        .await
        .unwrap();
    let indices2 = vec![
        IndexMetadata {
            name: "crate".to_string(),
            vers: "1.0.0".to_string(),
            deps: vec![],
            cksum: "cksum".to_string(),
            features: Default::default(),
            yanked: true,
            links: None,
            v: Some(1),
            features2: None,
        },
        IndexMetadata {
            name: "crate".to_string(),
            vers: "2.0.0".to_string(),
            deps: vec![],
            cksum: "cksum".to_string(),
            features: Default::default(),
            yanked: false,
            links: None,
            v: Some(1),
            features2: None,
        },
    ];
    test_db
        .db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked_str("crate".to_string()),
            "etag2",
            "last_modified2",
            None,
            &indices2,
        )
        .await
        .unwrap();

    let expected_prefetch = Prefetch {
        data: IndexMetadata::serialize_indices(&indices2)
            .map(|idx| idx.into_bytes())
            .unwrap(),
        etag: "etag2".to_string(),
        last_modified: "last_modified2".to_string(),
    };

    // Old etag
    let prefetch_state = test_db
        .db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("old_etag".to_string()),
            Some("last_modified".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(
        PrefetchState::NeedsUpdate(expected_prefetch.clone()),
        prefetch_state
    );

    // Old last_modified
    let prefetch_state = test_db
        .db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("old_last_modified".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(
        PrefetchState::NeedsUpdate(expected_prefetch.clone()),
        prefetch_state
    );

    // Old etag and last_modified
    let prefetch_state = test_db
        .db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("old_etag".to_string()),
            Some("old_last_modified".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(
        PrefetchState::NeedsUpdate(expected_prefetch),
        prefetch_state
    );
}

#[tokio::test]
async fn un_yank_crate() {
    let test_db = TestDB::new().await;
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_db
        .db
        .test_add_crate(
            "crate",
            "admin",
            &Version::from_unchecked_str("1.0.0"),
            &created,
        )
        .await
        .unwrap();

    // Yank the version
    test_db
        .db
        .yank_crate(
            &NormalizedName::from_unchecked_str("crate"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    // Test if yanked
    let ci = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked_str("crate"))
        .await
        .unwrap();
    assert!(
        ci.versions
            .iter()
            .find(|v| v.version == "1.0.0")
            .unwrap()
            .yanked
    );

    // Unyank the version
    test_db
        .db
        .unyank_crate(
            &NormalizedName::from_unchecked_str("crate"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    // Test if unyanked
    let ci = test_db
        .db
        .get_crate_data(&NormalizedName::from_unchecked_str("crate"))
        .await
        .unwrap();
    assert!(
        !ci.versions
            .iter()
            .find(|v| v.version == "1.0.0")
            .unwrap()
            .yanked
    );
}

#[tokio::test]
async fn test_get_last_updated_crate_works() {
    let test_db = TestDB::new().await;

    let created1 = DateTime::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap();
    let created1 = DateTime::<Utc>::from(created1);

    test_db.db.test_add_crate(
        "my_crate",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created1,
    )
    .await
    .unwrap();
    
    let created2 = DateTime::parse_from_rfc3339("2021-02-01T00:00:00Z").unwrap();
    let created2 = DateTime::<Utc>::from(created2);

    test_db.db.test_add_crate(
        "my_crate",
        "admin",
        &Version::from_unchecked_str("2.0.0"),
        &created2,
    )
    .await
    .unwrap();

    let created3 = DateTime::parse_from_rfc3339("2021-03-01T00:00:00Z").unwrap();
    let created3 = DateTime::<Utc>::from(created3);

    test_db.db.test_add_crate(
        "my_crate2",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created3,
    ).await.unwrap();
    

    let last_updated = test_db.db.get_last_updated_crate().await.unwrap().unwrap();

    assert_eq!(String::from("my_crate2"), last_updated.0.to_string());
}

#[tokio::test]
async fn test_get_last_updated_crate_empty() {
    let test_db = TestDB::new().await;

    let last_updated = test_db.db.get_last_updated_crate().await.unwrap();

    assert_eq!(None, last_updated);
}
