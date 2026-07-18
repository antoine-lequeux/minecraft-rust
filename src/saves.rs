use std::{collections::HashMap, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    chunks::Map,
    types::{BlockType, ChunkPos},
};

#[derive(Serialize, Deserialize)]
pub struct SaveFile
{
    pub name: String,
    pub seed: u32,
    pub player_position: [f32; 3],
    pub player_yaw: f32,
    pub player_pitch: f32,
    pub edits: Vec<(ChunkPos, Vec<(usize, BlockType)>)>,
}

#[derive(Clone, Debug, Default)]
pub struct SaveMetadata
{
    pub file_name: String,
    pub world_name: String,
}

pub fn save_game(map: &Map)
{
    let path = Path::new("saves");
    if !path.exists()
    {
        let _ = fs::create_dir(path);
    }

    // Convert HashMap to Vec for serialization.
    let mut edits = Vec::new();
    for (chunk_pos, modifs) in &map.modified
    {
        let mut chunk_edits = Vec::new();
        for (&index, &block) in modifs
        {
            chunk_edits.push((index, block));
        }
        edits.push((*chunk_pos, chunk_edits));
    }

    let save_data = SaveFile {
        name: map.world_name.clone(),
        seed: map.seed,
        player_position: map.player_position,
        player_yaw: map.player_yaw,
        player_pitch: map.player_pitch,
        edits,
    };

    let save_json = serde_json::to_string_pretty(&save_data).unwrap();

    let file_path = format!("saves/{}.json", map.save_file_name);
    let _ = fs::write(file_path, save_json);
}

pub fn load_game(file_name: &str) -> Option<Map>
{
    let file_path = format!("saves/{}.json", file_name);
    if let Ok(json) = fs::read_to_string(file_path)
    {
        if let Ok(save_data) = serde_json::from_str::<SaveFile>(&json)
        {
            let mut modified = HashMap::new();
            for (chunk_pos, chunk_edits) in save_data.edits
            {
                let mut modifs = HashMap::new();
                for (index, block) in chunk_edits
                {
                    modifs.insert(index, block);
                }
                modified.insert(chunk_pos, modifs);
            }

            return Some(Map {
                seed: save_data.seed,
                world_name: save_data.name,
                save_file_name: file_name.to_string(),
                player_position: save_data.player_position,
                player_yaw: save_data.player_yaw,
                player_pitch: save_data.player_pitch,
                modified,
            });
        }
    }
    None
}

pub fn get_saves() -> Vec<SaveMetadata>
{
    let mut saves = Vec::new();
    let path = Path::new("saves");
    if path.exists()
    {
        if let Ok(entries) = fs::read_dir(path)
        {
            for entry in entries.flatten()
            {
                if let Ok(file_type) = entry.file_type()
                {
                    if file_type.is_file()
                    {
                        let file_name = entry.file_name().into_string().unwrap();
                        if file_name.ends_with(".json")
                        {
                            let base_name = file_name.strip_suffix(".json").unwrap().to_string();
                            if let Ok(json) = fs::read_to_string(entry.path())
                            {
                                if let Ok(save_data) = serde_json::from_str::<SaveFile>(&json)
                                {
                                    saves.push(SaveMetadata { file_name: base_name, world_name: save_data.name });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    saves.sort_by(|a, b| {
        a.file_name
            .len()
            .cmp(&b.file_name.len())
            .then(a.file_name.cmp(&b.file_name))
    });

    return saves;
}

pub fn generate_new_save_file_name() -> String
{
    let saves = get_saves();
    let mut i = 1;
    loop
    {
        let name = format!("world{}", i);
        if !saves.iter().any(|s| s.file_name == name)
        {
            return name;
        }
        i += 1;
    }
}
