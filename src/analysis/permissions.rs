//! Manifest permissions whose API family never appears in the code.
//! Each one is a privacy-review and Play-Store liability for a
//! capability nobody uses. Only permissions with a known API mapping
//! are checked — custom or exotic permissions are unverifiable from
//! sources alone. A mention of the permission constant itself
//! (checkSelfPermission) counts as usage.

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct UnusedPermission {
    pub name: String,
    pub manifest: PathBuf,
}

/// Permission (last segment) → API tokens that prove the capability
/// is exercised somewhere.
const PERMISSION_APIS: &[(&str, &[&str])] = &[
    (
        "CAMERA",
        &[
            "CameraManager",
            "camera2",
            "CameraX",
            "ImageCapture",
            "Camera.open",
        ],
    ),
    ("RECORD_AUDIO", &["MediaRecorder", "AudioRecord"]),
    (
        "ACCESS_FINE_LOCATION",
        &[
            "LocationManager",
            "FusedLocationProvider",
            "getLastLocation",
            "requestLocationUpdates",
        ],
    ),
    (
        "ACCESS_COARSE_LOCATION",
        &[
            "LocationManager",
            "FusedLocationProvider",
            "getLastLocation",
            "requestLocationUpdates",
        ],
    ),
    ("READ_CONTACTS", &["ContactsContract"]),
    ("WRITE_CONTACTS", &["ContactsContract"]),
    ("BLUETOOTH", &["BluetoothAdapter", "BluetoothManager"]),
    (
        "BLUETOOTH_CONNECT",
        &["BluetoothAdapter", "BluetoothManager", "BluetoothGatt"],
    ),
    ("NFC", &["NfcAdapter"]),
    ("VIBRATE", &["Vibrator", "VibratorManager"]),
    ("WAKE_LOCK", &["PowerManager", "WakeLock"]),
    (
        "ACCESS_NETWORK_STATE",
        &["ConnectivityManager", "NetworkCapabilities"],
    ),
    (
        "POST_NOTIFICATIONS",
        &["NotificationManager", "NotificationCompat"],
    ),
    (
        "INTERNET",
        &[
            "OkHttp",
            "Retrofit",
            "HttpURLConnection",
            "WebView",
            "Socket",
            "Volley",
            "Ktor",
            "HttpClient",
        ],
    ),
    (
        "READ_EXTERNAL_STORAGE",
        &["getExternalStorage", "MediaStore"],
    ),
    (
        "WRITE_EXTERNAL_STORAGE",
        &["getExternalStorage", "MediaStore"],
    ),
];

static PERMISSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<uses-permission[^>]*android:name="([\w.]+)""#).unwrap());

/// None when the tree has no AndroidManifest.xml at all.
pub fn unused_permissions(root: &Path) -> Option<Vec<UnusedPermission>> {
    let mut declared: Vec<(String, PathBuf)> = Vec::new();
    let mut corpus = String::new();
    let mut found_manifest = false;

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || name == "build" || name == "node_modules")
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        if name == "AndroidManifest.xml" {
            found_manifest = true;
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for cap in PERMISSION_RE.captures_iter(&content) {
                    declared.push((cap[1].to_string(), entry.path().to_path_buf()));
                }
            }
        } else if name.ends_with(".kt") || name.ends_with(".java") {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                corpus.push_str(&content);
                corpus.push('\n');
            }
        }
    }

    if !found_manifest {
        return None;
    }

    let mut unused: Vec<UnusedPermission> = declared
        .into_iter()
        .filter(|(name, _)| {
            let last = name.rsplit('.').next().unwrap_or(name);
            let Some((_, tokens)) = PERMISSION_APIS.iter().find(|(perm, _)| *perm == last) else {
                return false; // unknown permission: unverifiable
            };
            // the permission constant itself showing up is intent enough
            if corpus.contains(name.as_str()) || corpus.contains(&format!("permission.{last}")) {
                return false;
            }
            !tokens.iter().any(|token| corpus.contains(token))
        })
        .map(|(name, manifest)| UnusedPermission { name, manifest })
        .collect();
    unused.sort_by(|a, b| a.name.cmp(&b.name));
    unused.dedup_by(|a, b| a.name == b.name);
    Some(unused)
}
