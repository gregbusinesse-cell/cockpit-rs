//! Les commandes communes à tous les écrans : état de la fenêtre, réglages, collecte, sessions, liens.
use crate::hote::{Pages, AGENT};
use crate::Etat;
use noyau::collecte::collecteur::REGLAGE_PAUSE;
use noyau::modeles::Passage;
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

/// Le résultat d'une commande ; l'erreur arrive en texte à l'interface, qui l'affiche.
pub type Rep<T> = Result<T, String>;

pub fn texte<E: std::fmt::Display>(e: E) -> String { e.to_string() }

/// Les réglages que la fenêtre relit au lancement.
const REGLAGES: [&str; 5] = ["ecran", "business", "boutique", "accueil.reseau", "accueil.plan"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatFenetre {
    version: u64,
    en_cours: Vec<String>,
    pause: bool,
    reglages: BTreeMap<String, String>,
    plateforme: &'static str,
    businesses: Vec<BusinessFenetre>,
    sources: Vec<SourceFenetre>,
    a_traiter: usize,
    /// Le dossier des images du cache : l'interface y lit `<clé>` avec `:` et `/` changés en `_`.
    miniatures: String,
}

/// Un business tel que la barre du haut le montre.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessFenetre {
    id: String,
    nom: String,
    lettre: String,
    teinte: String,
    app_store_id: Option<String>,
}

/// Une source et son dernier passage, pour les points de la barre du haut.
#[derive(Serialize)]
pub struct SourceFenetre {
    id: &'static str,
    nom: &'static str,
    passage: Option<Passage>,
}

/// Le canal du navigateur caché (`plugin:veille|message`), ouvert aux pages distantes par capabilities/veille.json.
#[tauri::command]
pub fn message(pages: State<'_, Pages>, page: String, cle: String, texte: String) { pages.recevoir(&page, &cle, texte) }

#[tauri::command]
pub async fn etat(etat: State<'_, Etat>) -> Rep<EtatFenetre> {
    let mut reglages = BTreeMap::new();
    for cle in REGLAGES {
        if let Some(v) = etat.store.reglage(cle).map_err(texte)? {
            reglages.insert(cle.to_string(), v);
        }
    }
    let mut en_cours: Vec<String> = etat.collecteur.en_cours().into_iter().collect();
    en_cours.sort();
    let businesses = etat.store.businesses().map_err(texte)?.into_iter()
        .map(|b| BusinessFenetre { id: b.id, nom: b.nom, lettre: b.lettre, teinte: b.teinte, app_store_id: b.config.app_store_id })
        .collect();
    let mut sources = Vec::new();
    for s in &etat.collecteur.sources {
        sources.push(SourceFenetre { id: s.id(), nom: s.nom(), passage: etat.store.dernier_passage(s.id()).map_err(texte)? });
    }
    Ok(EtatFenetre {
        version: etat.store.version(),
        en_cours,
        pause: etat.store.reglage(REGLAGE_PAUSE).map_err(texte)?.as_deref() == Some("1"),
        reglages,
        plateforme: std::env::consts::OS,
        businesses,
        sources,
        a_traiter: crate::ecrans::resume_tray(&etat.store).a_traiter,
        miniatures: noyau::collecte::miniatures::dossier().to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub async fn reglage(etat: State<'_, Etat>, cle: String) -> Rep<Option<String>> { etat.store.reglage(&cle).map_err(texte) }

/// Un réglage de la fenêtre (écran, business choisi…) : pas de nouvelle version de la base.
#[tauri::command]
pub async fn regler(etat: State<'_, Etat>, cle: String, valeur: Option<String>) -> Rep<()> {
    etat.store.regler(&cle, valeur.as_deref()).map_err(texte)
}

/// Un réglage qui change ce que montrent les écrans (notifications coupées, objectif…) : la base change de version.
#[tauri::command]
pub async fn ecrire_reglage(etat: State<'_, Etat>, cle: String, valeur: Option<String>) -> Rep<()> {
    etat.store.regler(&cle, valeur.as_deref()).map_err(texte)?;
    etat.store.a_change();
    Ok(())
}

#[tauri::command]
pub async fn pause(etat: State<'_, Etat>, oui: bool) -> Rep<()> {
    etat.store.regler(REGLAGE_PAUSE, oui.then_some("1")).map_err(texte)?;
    etat.store.a_change();
    Ok(())
}

/// Relance une source, ou toutes.
#[tauri::command]
pub async fn relancer(app: AppHandle, etat: State<'_, Etat>, source: Option<String>) -> Rep<()> {
    etat.collecteur.relancer(source.as_deref());
    if source.is_none() {
        crate::maj::verifier(app);
    }
    Ok(())
}

/// Les sites où l'on se connecte dans l'app quand Safari n'est pas là (Windows) : leur page de connexion.
pub const SITES: [(&str, &str, &str); 3] = [
    ("tiktok", "TikTok", "https://www.tiktok.com/login"),
    ("instagram", "Instagram", "https://www.instagram.com/accounts/login/"),
    ("facebook", "Facebook", "https://www.facebook.com/login/"),
];

/// Ouvre la fenêtre de connexion d'un site ; sa session reste dans le magasin de cookies de l'app.
#[tauri::command]
pub async fn session(app: AppHandle, site: String) -> Rep<()> {
    let (id, nom, url) = SITES.iter().find(|(id, _, _)| *id == site).ok_or_else(|| format!("site inconnu : {site}"))?;
    let libelle = format!("sessions-{id}");
    if let Some(f) = app.get_webview_window(&libelle) {
        return f.set_focus().map_err(texte);
    }
    WebviewWindowBuilder::new(&app, libelle, WebviewUrl::External(url.parse().map_err(texte)?))
        .title(format!("Se connecter à {nom}"))
        .inner_size(480.0, 760.0)
        .user_agent(AGENT)
        .build()
        .map_err(texte)?;
    Ok(())
}

/// Ouvre un lien dans le navigateur de l'ordinateur.
#[tauri::command]
pub async fn lien(app: AppHandle, url: String) -> Rep<()> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(format!("lien refusé : {url}"));
    }
    app.opener().open_url(url, None::<&str>).map_err(texte)
}
