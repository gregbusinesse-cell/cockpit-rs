//! Version Zest : mises à jour automatiques et graine par défaut.
//!
//! - Au démarrage, toutes les 30 minutes et à chaque « Tout relancer » (bouton ↻, Ctrl+R, F5), l'app regarde
//!   `latest.json` de la dernière release GitHub (gregbusinesse-cell/cockpit-rs). S'il y a une version plus récente,
//!   elle la télécharge, vérifie sa signature et l'installe, puis redémarre.
//! - Au premier lancement, sans graine, on écrit celle de Zest (aucune clé dedans, seulement des identifiants publics).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const GRAINE_ZEST: &str = include_str!("../graine_zest.json");

static EN_COURS: AtomicBool = AtomicBool::new(false);

pub fn graine_par_defaut() {
    let chemin = noyau::chemins::graine();
    if chemin.exists() {
        return;
    }
    if let Some(dossier) = chemin.parent() {
        let _ = std::fs::create_dir_all(dossier);
    }
    if let Err(e) = std::fs::write(&chemin, GRAINE_ZEST) {
        eprintln!("graine : {e}");
    }
}

pub fn surveiller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            chercher(&app).await;
            tokio::time::sleep(Duration::from_secs(30 * 60)).await;
        }
    });
}

pub fn verifier(app: AppHandle) {
    tauri::async_runtime::spawn(async move { chercher(&app).await });
}

async fn chercher(app: &AppHandle) {
    if EN_COURS.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = installer(app).await {
        eprintln!("mise à jour : {e}");
    }
    EN_COURS.store(false, Ordering::SeqCst);
}

async fn installer(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let Some(maj) = app.updater()?.check().await? else {
        return Ok(());
    };
    maj.download_and_install(|_, _| {}, || {}).await?;
    app.restart();
}
