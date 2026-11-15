// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod encoder_decoder;
mod heap;
mod huffman;
mod async_counter_stream;

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use serde::Serialize;
use tokio::select;
use rfd::FileDialog;
use crate::async_counter_stream::{ByteCounterReadStream, ByteCounterWriteStream};
use crate::encoder_decoder::Decoder;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationReport {
    input_bytes: u32,
    output_bytes: u32,
    elapsed_ms: u64,
}

#[tauri::command]
async fn compress_file(input_path: String, output_path: String, on_progress: tauri::ipc::Channel<u32>, ) -> Result<OperationReport, String>{
    let start_time = std::time::Instant::now();
    let (text, input_bytes) = tokio::task::spawn_blocking(move || -> Result<(String, u32), String>{
        // read file by tokio
       let f = File::open(&input_path).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(f);
        let mut text = String::new();
        // read file
        reader.read_to_string(&mut text).map_err(|e| e.to_string())?;
        let n = text.len() as u32;
        Ok((text, n))
    }).await.map_err(|e| e.to_string())??;

    // initialize
    // create the output file
    let result_file = File::create(output_path).map_err(|e|e.to_string())?;
    // use ByteCounter Stream to get the num of bytes that have been encoded
    let writer = ByteCounterWriteStream::new(BufWriter::new(result_file));
    // get the handle (Arc<AtomicUsize>) to fetch the num of encoded byte
    let progress_handler = writer.get_handle();
    // the watcher takes its own handle, so the original stays usable after encoding finishes
    let progress_handler_for_watcher = progress_handler.clone();
    // initialize the encoder
    // `Encoder::open` returns None for empty input; treat that as an error.
    let Some(encoder) = encoder_decoder::Encoder::open(text, writer) else{
        return Err("Error: input file is empty".to_string());
    };
    let expected_output_total_byte_num = encoder.get_total_byte_size();
    //encode task function generator
    let encoding_task =tokio::task::spawn_blocking (move || {
        encoder.encode().map_err(|e| e.to_string())
    });
    //this task will not stop until the encode task drop it.
    let progress_watcher_task = async move {
        loop{
            let written = progress_handler_for_watcher.load(std::sync::atomic::Ordering::Relaxed);
            let progress = (written * 100 / expected_output_total_byte_num).min(100) as u32;
            let _ = on_progress.send(progress);
            println!("{}", progress);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    };
    //run the encode task
    select! {
        //when the encode finishes, select! macro will drop the watcher task
        encode = encoding_task => encode.map_err(|e| e.to_string())??,
        //do not use break statement here, which will interrupt the encode task
        _ = progress_watcher_task => unreachable!("watching task should never complete until encoding finishes"),
    }
    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    // the estimate is only the denominator for progress; report what was actually written
    let output_bytes = progress_handler.load(std::sync::atomic::Ordering::Relaxed) as u32;
    println!("{} {} {}", input_bytes, output_bytes, elapsed_ms);
    Ok(OperationReport {
        input_bytes,
        output_bytes,
        elapsed_ms,
    })
}

#[tauri::command]
async fn decompress_file(input_path: String, output_path: String, on_progress: tauri::ipc::Channel<u32>, ) -> Result<OperationReport, String>{
    let start_time = std::time::Instant::now();
    //
    let input_file = tokio::task::spawn_blocking(move || -> Result<File, String>{
        // read file by tokio
        File::open(&input_path).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())??;
    let expected_input_total_byte_num = input_file.metadata().map_err(|e|{e.to_string()})?.len() as u32;
    // use ByteCounterStream to get the num of encoded byte
    let reader = ByteCounterReadStream::new(BufReader::new(input_file));
    // get the handle (Arc<AtomicUsize>) to fetch the num of encoded byte
    let progress_handler = reader.get_handle();
    let decoder = Decoder::open(reader);
    let decoding_task = tokio::task::spawn_blocking (move || async move {
        decoder.decode().map_err(|e| e.to_string())
    });

    let progress_watcher_task =async move {
        loop{
            let progress = (progress_handler.load(std::sync::atomic::Ordering::Relaxed) * 100 ) as u32/ expected_input_total_byte_num;
            let _ = on_progress.send(progress);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    };
    //run the encode task
    let decode = select! {
        //when the encode finishes, select! macro will drop the watcher task
        decode = decoding_task => decode,
        //do not use break statement here, which will interrupt the encode task
        _ = progress_watcher_task => unreachable!("watching task should never complete until encoding finishes"),
    }.map_err(|e| e.to_string())?.await?;
    let output_bytes = decode.as_bytes().len() as u32;
    // write the decoded text
    let output_file = File::create(&output_path).map_err(|e|e.to_string())?;
    let mut writer = BufWriter::new(output_file);
    writer.write_all(decode.as_bytes()).map_err(|e|e.to_string())?;
    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    println!("{} {} {}", expected_input_total_byte_num, output_bytes, elapsed_ms);
    Ok(OperationReport {
        input_bytes : expected_input_total_byte_num,
        output_bytes,
        elapsed_ms,
    })
}
#[tauri::command]
async fn pick_output_path(expected_file_name: String) -> Result<Option<String>, String>{
    let extension= match expected_file_name.rsplit_once('.'){
        Some((_, ext)) => ext,
        None => ""
    };
    println!("Picking output file: {}", expected_file_name);
    let Some(path_buf) =  FileDialog::new().set_file_name(&expected_file_name).add_filter("File Name", &[extension])
        .save_file() else{
        return Ok(None);
    };
    let Some(path_str) = path_buf.to_str() else{
        return Err("Error: selected file name contains invalid UTF-8 characters".to_string());
    };
    Ok(Some(String::from(path_str)))
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![compress_file, decompress_file, pick_output_path])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


