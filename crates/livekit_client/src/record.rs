use std::{
    env,
    num::NonZero,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::DeviceId;
use cpal::traits::{DeviceTrait, StreamTrait};
use rodio::{buffer::SamplesBuffer, conversions::SampleTypeConverter};
use util::ResultExt;

pub struct CaptureInput {
    pub name: String,
    pub input_device: Option<DeviceId>,
    config: cpal::SupportedStreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
    _stream: cpal::Stream,
}

impl CaptureInput {
    pub fn start(input_device: Option<DeviceId>) -> anyhow::Result<Self> {
        let (device, config) = crate::default_device(true, input_device.as_ref())?;
        let name = device
            .description()
            .map(|desc| desc.name().to_string())
            .unwrap_or("<unknown>".to_string());
        log::info!("正在使用麦克风: {}", name);

        let samples = Arc::new(Mutex::new(Vec::new()));
        let stream = start_capture(device, config.clone(), samples.clone())?;

        Ok(Self {
            name,
            input_device,
            _stream: stream,
            config,
            samples,
        })
    }

    pub fn finish(self) -> Result<PathBuf> {
        let name = self.name;
        let mut path = env::current_dir().context("无法获取当前目录")?;
        path.push(&format!("test_recording_{name}.wav"));
        log::info!("测试录音已写入:{}", path.display());
        write_out(self.samples, self.config, &path)?;
        Ok(path)
    }
}

fn start_capture(
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
) -> Result<cpal::Stream> {
    let stream = device
        .build_input_stream_raw(
            &config.config(),
            config.sample_format(),
            move |data, _: &_| {
                let data = crate::get_sample_data(config.sample_format(), data).log_err();
                let Some(data) = data else {
                    return;
                };
                samples
                    .try_lock()
                    .expect("仅在流结束后锁定")
                    .extend_from_slice(&data);
            },
            |err| log::error!("捕获音频轨道时出错: {:?}", err),
            Some(Duration::from_millis(100)),
        )
        .context("无法构建输入流")?;

    stream.play()?;
    Ok(stream)
}

fn write_out(
    samples: Arc<Mutex<Vec<i16>>>,
    config: cpal::SupportedStreamConfig,
    path: &Path,
) -> Result<()> {
    let samples = std::mem::take(
        &mut *samples
            .try_lock()
            .expect("流已结束,回调无法持有锁"),
    );
    let samples: Vec<f32> = SampleTypeConverter::<_, f32>::new(samples.into_iter()).collect();
    let mut samples = SamplesBuffer::new(
        NonZero::new(config.channels()).expect("配置声道数永远不会为零"),
        NonZero::new(config.sample_rate()).expect("配置采样率永远不会为零"),
        samples,
    );
    match rodio::wav_to_file(&mut samples, path) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("写入 WAV 文件失败:{}", e)),
    }
}
