fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    let mut did = false;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshots" => {
                let dir = args.get(i + 1).cloned().unwrap_or_else(|| "screens".into());
                if let Err(e) = cubicraft_desktop::run_screenshots(&dir) {
                    eprintln!("screenshots failed: {e}");
                    std::process::exit(1);
                }
                did = true;
                i += 2;
            }
            "--frames" => {
                let dir = args.get(i + 1).cloned().unwrap_or_else(|| "frames".into());
                let n: usize = args
                    .get(i + 2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(96);
                if let Err(e) = cubicraft_desktop::run_frames(&dir, n) {
                    eprintln!("frames failed: {e}");
                    std::process::exit(1);
                }
                did = true;
                i += 3;
            }
            "--check" => {
                if let Err(e) = cubicraft_desktop::run_check() {
                    eprintln!("check failed: {e}");
                    std::process::exit(1);
                }
                did = true;
                i += 1;
            }
            other => {
                eprintln!("unknown arg {other}");
                eprintln!("usage: cubicraft-desktop [--screenshots DIR | --frames DIR N | --check]");
                std::process::exit(2);
            }
        }
    }
    if did {
        return;
    }
    let event_loop = winit::event_loop::EventLoop::builder().build().unwrap();
    if let Err(e) = cubicraft_desktop::run_window(event_loop) {
        eprintln!("run error: {e}");
        std::process::exit(1);
    }
}
