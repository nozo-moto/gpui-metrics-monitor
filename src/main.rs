mod metrics;
mod ui;

use std::{
    sync::{Arc, Mutex},
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
        let collectors = Arc::new(Mutex::new(Collectors {
            system: SystemCollector::new(),
            gpu: GpuCollector::new(),
        }));

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
                let collectors = collectors.clone();
                let (system, gpus) = cx
                    .background_executor()
                    .spawn(async move {
                        let mut collectors = collectors.lock().expect("collector lock poisoned");
                        (collectors.system.sample(), collectors.gpu.sample())
                    })
                    .await;

                if cx
                    .update_entity(&dashboard, |view, cx| {
                        view.update(system, gpus);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }

                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        })
        .detach();

        cx.activate(true);
    });
}
