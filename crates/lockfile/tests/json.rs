use lockfile::parse_json;

const APP: &str = include_str!("fixtures/laravel-ish.composer.json");

#[test]
fn parses_require_and_dev() {
    let cj = parse_json(APP).expect("deve parsear");
    assert_eq!(cj.require.get("laravel/framework").map(String::as_str), Some("^11.0"));
    assert_eq!(cj.require.get("php").map(String::as_str), Some("^8.2"));
    assert_eq!(cj.require_dev.get("phpunit/phpunit").map(String::as_str), Some("^11.0"));
}

#[test]
fn parses_psr4_string_and_array() {
    let cj = parse_json(APP).expect("deve parsear");
    let psr4 = &cj.autoload.psr4;
    assert_eq!(psr4.get("App\\").unwrap().as_slice(), &["app/".to_string()]);
    assert_eq!(
        psr4.get("Database\\Factories\\").unwrap().as_slice(),
        &["database/factories/".to_string(), "extra/factories/".to_string()]
    );
}

#[test]
fn parses_files_classmap_scripts_bin() {
    let cj = parse_json(APP).expect("deve parsear");
    assert_eq!(cj.autoload.files, vec!["app/helpers.php".to_string()]);
    assert_eq!(cj.autoload.classmap, vec!["app/Legacy".to_string()]);
    assert_eq!(cj.bin, vec!["bin/console".to_string()]);
    let pad = cj.scripts.get("post-autoload-dump").expect("tem hook");
    assert_eq!(pad.len(), 2);
    assert_eq!(pad[1], "@php artisan package:discover --ansi");
}

#[test]
fn tolerates_null_bin_and_scripts() {
    let cj = parse_json(r#"{"bin": null, "scripts": null}"#).expect("null deve virar vazio");
    assert!(cj.bin.is_empty());
    assert!(cj.scripts.is_empty());
}

#[test]
fn bin_accepts_single_string() {
    let cj = parse_json(r#"{"bin": "bin/console"}"#).expect("bin string");
    assert_eq!(cj.bin, vec!["bin/console".to_string()]);
}

#[test]
fn empty_object_is_all_defaults() {
    let cj = parse_json("{}").expect("vazio ok");
    assert!(cj.name.is_empty());
    assert!(cj.require.is_empty());
    assert!(cj.autoload.psr4.is_empty());
}
