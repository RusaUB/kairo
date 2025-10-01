use std::fs;
use rusqlite::{Connection, params};
use tauri::{AppHandle, Manager};
use serde::{Serialize, Deserialize};

use crate::cred::Api;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LlmProvider {
    pub name: String,
    pub base_url: String,
    pub api: Api,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Agent {
    pub name: String,
    pub provider: LlmProvider,
    pub system_prompt: String,
    pub model: String,
}

fn open_db(app: &AppHandle) -> Result<Connection, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db_path = dir.join("kairo.sqlite3");

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS llm_providers (
          id         INTEGER PRIMARY KEY AUTOINCREMENT,
          name       TEXT    NOT NULL UNIQUE,
          api_enc    BLOB    NOT NULL,                        
          base_url   TEXT    NOT NULL,
          created_at TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP),
          updated_at TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP)
        );

        CREATE TABLE IF NOT EXISTS agents (
          id            INTEGER PRIMARY KEY AUTOINCREMENT,
          name          TEXT    NOT NULL UNIQUE,
          provider_id   INTEGER NOT NULL REFERENCES llm_providers(id)
                         ON UPDATE CASCADE ON DELETE RESTRICT,
          model         TEXT    NOT NULL,
          system_prompt TEXT    NOT NULL,
          created_at    TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP),
          updated_at    TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP)
        );

        CREATE INDEX IF NOT EXISTS idx_agents_provider_id ON agents(provider_id);
    "#).map_err(|e| e.to_string())?;

    Ok(conn)
}

#[tauri::command]
pub(crate) fn add_llmprovider(app: AppHandle, provider: LlmProvider) -> Result<(), String> {
    let conn = open_db(&app)?;
    let api_enc = provider.api.encrypt()?;

    conn.execute(
        r#"
        INSERT INTO llm_providers (name, api_enc, base_url)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(name) DO UPDATE
          SET api_enc    = excluded.api_enc,
              base_url   = excluded.base_url,
              updated_at = CURRENT_TIMESTAMP
        "#,
        params![provider.name, api_enc, provider.base_url],
    ).map_err(|e| e.to_string())?;

    Ok(())
}


#[tauri::command]
pub(crate) fn get_llmprovider(app: AppHandle, name: String) -> Result<LlmProvider, String> {
    let conn = open_db(&app)?;
    let (name_out, base_url, api_enc): (String, String, Vec<u8>) = conn.query_row(
        "SELECT name, base_url, api_enc FROM llm_providers WHERE name = ?1",
        params![name],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| e.to_string())?;

    let api = Api::decrypt(&api_enc)?;
    Ok(LlmProvider { name: name_out, base_url, api })
}

#[tauri::command]
pub(crate) fn get_all_llmproviders(app: AppHandle) -> Result<Vec<LlmProvider>, String> {
    let conn = open_db(&app)?;

    let mut stmt = conn
        .prepare("SELECT name, base_url, api_enc FROM llm_providers ORDER BY name ASC")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let base_url: String = row.get(1)?;
            let api_enc: Vec<u8> = row.get(2)?;
            Ok((name, base_url, api_enc))
        })
        .map_err(|e| e.to_string())?;

    let mut providers = Vec::new();
    for row in rows {
        let (name, base_url, api_enc) = row.map_err(|e| e.to_string())?;
        let api = Api::decrypt(&api_enc)?;
        providers.push(LlmProvider { name, base_url, api });
    }

    Ok(providers)
}

 
#[tauri::command]
pub(crate) fn add_agent(app: AppHandle, agent: Agent) -> Result<(), String> {
    let conn = open_db(&app)?;

    let api_enc = agent.provider.api.encrypt()?;
    conn.execute(
        r#"
        INSERT INTO llm_providers (name, api_enc)
        VALUES (?1, ?2)
        ON CONFLICT(name) DO UPDATE
          SET api_enc   = excluded.api_enc,
              updated_at = CURRENT_TIMESTAMP
        "#,
        params![agent.provider.name, api_enc],
    ).map_err(|e| e.to_string())?;

    let provider_id: i64 = conn.query_row(
        "SELECT id FROM llm_providers WHERE name = ?1",
        params![agent.provider.name],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    conn.execute(
        r#"
        INSERT INTO agents (name, provider_id, model, system_prompt)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(name) DO UPDATE
          SET provider_id   = excluded.provider_id,
              model         = excluded.model,
              system_prompt = excluded.system_prompt,
              updated_at    = CURRENT_TIMESTAMP
        "#,
        params![agent.name, provider_id, agent.model, agent.system_prompt],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub(crate) fn get_agent(app: AppHandle, name: String) -> Result<Agent, String> {
    let conn = open_db(&app)?;

    let (agent_name, model, system_prompt, provider_name, base_url, api_enc): (String, String, String, String, String, Vec<u8>) =
        conn.query_row(
            r#"
            SELECT
                a.name,
                a.model,
                a.system_prompt,
                p.name,
                p.base_url,
                p.api_enc
            FROM agents a
            JOIN llm_providers p ON p.id = a.provider_id
            WHERE a.name = ?1
            "#,
            params![name],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => "Agent not found".to_string(),
            _ => e.to_string(),
        })?;

    let api = Api::decrypt(&api_enc)?;
    let provider = LlmProvider { name: provider_name, base_url, api };

    Ok(Agent { name: agent_name, provider, system_prompt, model })
}


#[tauri::command]
pub(crate) fn get_all_agents(app: AppHandle) -> Result<Vec<Agent>, String> {
    let conn = open_db(&app)?;

    let mut stmt = conn.prepare(
        r#"
        SELECT
            a.name,
            a.model,
            a.system_prompt,
            p.name,
            p.base_url,
            p.api_enc
        FROM agents a
        JOIN llm_providers p ON p.id = a.provider_id
        ORDER BY a.name ASC
        "#
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        let agent_name: String = row.get(0)?;
        let model: String = row.get(1)?;
        let system_prompt: String = row.get(2)?;
        let provider_name: String = row.get(3)?;
        let base_url: String = row.get(4)?;
        let api_enc: Vec<u8> = row.get(5)?;
        Ok((agent_name, model, system_prompt, provider_name, base_url, api_enc))
    }).map_err(|e| e.to_string())?;

    let mut agents = Vec::new();
    for row in rows {
        let (agent_name, model, system_prompt, provider_name, base_url, api_enc) =
            row.map_err(|e| e.to_string())?;
        let api = Api::decrypt(&api_enc)?;
        let provider = LlmProvider { name: provider_name, base_url, api };
        agents.push(Agent { name: agent_name, provider, system_prompt, model });
    }

    Ok(agents)
}
