/**
 * 前端与 Rust 之间的接口层 —— UI 只依赖这个文件。
 *
 * 后端（src-tauri）需要提供两个命令，示意签名：
 *
 *     #[tauri::command]
 *     async fn compress_file(
 *         input_path: String,
 *         output_path: String,
 *         on_progress: tauri::ipc::Channel<u32>,
 *     ) -> Result<OperationReport, String>;
 *
 *     #[tauri::command]
 *     async fn decompress_file(
 *         input_path: String,
 *         output_path: String,
 *         on_progress: tauri::ipc::Channel<u32>,
 *     ) -> Result<OperationReport, String>;
 *
 * 返回体（serde camelCase）：
 *
 *     { "inputBytes": 1234, "outputBytes": 567, "elapsedMs": 42 }
 *
 * 约定：
 * 1. `on_progress` 传 0–100 的整数百分比；结束时建议再发一次 100。
 * 2. 失败时返回任意可序列化的错误（`String` 最省事），前端会原样显示。
 * 3. 两个命令都要登记进 `tauri::generate_handler![...]`。
 * 4. 参数名遵循 Tauri 的自动转换：
 *    前端 `inputPath` → Rust `input_path`，`onProgress` → `on_progress`。
 *
 * 如果后端最终用了别的命令名，只改下面的 COMMANDS 即可，UI 不用动。
 */
import { Channel, invoke } from "@tauri-apps/api/core";

export type Mode = "compress" | "decompress";

export interface OperationReport {
  /** 输入文件字节数 */
  inputBytes: number;
  /** 输出文件字节数 */
  outputBytes: number;
  /** 后端统计的耗时（毫秒） */
  elapsedMs: number;
}

export interface RunOptions {
  inputPath: string;
  outputPath: string;
  /** 后端每完成一段就回调一次，参数是 0–100 的百分比 */
  onProgress: (percent: number) => void;
}

const COMMANDS: Record<Mode, string> = {
  compress: "compress_file",
  decompress: "decompress_file",
};

/** 当前页面是否跑在 Tauri 窗口里（浏览器直接打开时为 false） */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function commandName(mode: Mode): string {
  return COMMANDS[mode];
}

/**
 * 执行一次压缩 / 解压。
 * 进度通过 Tauri 的 Channel 回传，不占用事件系统。
 */
export async function runOperation(
  mode: Mode,
  options: RunOptions,
): Promise<OperationReport> {
  const progress = new Channel<number>();
  progress.onmessage = (percent) => options.onProgress(percent);

  return invoke<OperationReport>(COMMANDS[mode], {
    inputPath: options.inputPath,
    outputPath: options.outputPath,
    onProgress: progress,
  });
}
