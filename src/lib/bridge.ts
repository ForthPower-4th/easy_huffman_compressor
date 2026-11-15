/**
 * 前端与 Rust 之间的接口层 —— UI 只依赖这个文件。
 *
 * 后端（src-tauri）需要提供以下命令，示意签名：
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
 * 输出路径选择命令（由 Rust 侧实现）：
 *
 *     #[tauri::command]
 *     async fn pick_output_path(expected_file_name: String) -> Result<Option<String>, String>;
 *
 * `expected_file_name` 是保存对话框的默认文件名，只含文件名、不含目录：
 * 压缩时是「输入文件名去掉后缀 + .huff」，解压时是「输入文件名去掉后缀 + .txt」。
 * 返回用户选中的完整路径；取消时返回 `None`（序列化为 null）或空字符串，
 * 前端会保持输出框原来的值不变。
 *
 * 约定：
 * 1. `on_progress` 传 0–100 的整数百分比；结束时建议再发一次 100。
 * 2. 失败时返回任意可序列化的错误（`String` 最省事），前端会原样显示。
 * 3. 这些命令都要登记进 `tauri::generate_handler![...]`。
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

/** 选择输出路径的 Rust 命令（由后端实现） */
export const PICK_OUTPUT_PATH_COMMAND = "pick_output_path";

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

/**
 * 打开系统文件选择器，返回用户选中的输出路径。
 *
 * 对应的 Rust 命令是 `pick_output_path(expected_file_name: String)`：
 *
 *     #[tauri::command]
 *     async fn pick_output_path(expected_file_name: String) -> Result<Option<String>, String>;
 *
 * `expectedFileName` 只传文件名（不含目录），由调用方按输入文件名和当前模式生成。
 * 取消选择时返回 null 或空字符串，本函数统一转成 null，调用方不修改输出框。
 */
export async function pickOutputPath(
  expectedFileName: string,
): Promise<string | null> {
  const picked = await invoke<string | null>(PICK_OUTPUT_PATH_COMMAND, {
    expectedFileName: expectedFileName.trim(),
  });
  if (typeof picked !== "string") return null;
  const path = picked.trim();
  return path.length > 0 ? path : null;
}
