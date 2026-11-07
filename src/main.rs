mod metrics;
mod ui;

use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use gpui::{App, AppContext, Application, Size, WindowBounds, WindowOptions, px};
use metrics::{gpu::GpuCollector, system::SystemCollector};
use ui::Dashboard;

struct Collectors {
    system: SystemCollector,
    gpu: GpuCollector,
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let dashboard = cx.new(|_| Dashboard::default());
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut collectors = Collectors {
                system: SystemCollector::new(),
                gpu: GpuCollector::new(),
            };
            loop {
                let readings = (collectors.system.sample(), collectors.gpu.sample());
                if sender.send(readings).is_err() {
                    break;
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
        let receiver = Arc::new(Mutex::new(receiver));

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(Size::new(px(1120.), px(820.)), cx)),
                window_min_size: Some(Size::new(px(760.), px(600.))),
                ..Default::default()
            },
            |_, _| dashboard.clone(),
        )
        .expect("could not open dashboard window");

        cx.spawn(async move |cx| {
            loop {
                let receiver = receiver.clone();
                let Ok((system, gpus)) = cx
                    .background_executor()
                    .spawn(async move { receiver.lock().expect("receiver lock poisoned").recv() })
                    .await
                else {
                    break;
                };

                if cx
                    .update_entity(&dashboard, |view, cx| {
                        view.update(system, gpus);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        cx.activate(true);
    });
}
