use crate::data::{Phonem, PreviewId, Video, YoutubeId};
use std::sync::Arc;

use ges::prelude::*;
use gst::ClockTime;
use gst_pbutils::{
    EncodingAudioProfileBuilder, EncodingContainerProfileBuilder, EncodingVideoProfileBuilder,
};

type BoxResult = Result<(), Box<dyn std::error::Error>>;

fn render_pipeline(
    pipeline: &ges::Pipeline,
    out_uri: &str,
    out_caps: Option<&gst::Caps>,
) -> BoxResult {
    let audio_profile = EncodingAudioProfileBuilder::new()
        .format(&gst::Caps::new_simple("audio/mpeg", &[]))
        .presence(0)
        .build()?;

    // Every videostream piped into the encodebin should be encoded using vp9.
    let video_profile = EncodingVideoProfileBuilder::new()
        .format(out_caps.unwrap_or(&gst::Caps::new_simple("video/x-h264", &[])))
        .presence(0)
        .build()?;

    // All streams are then finally combined into a mp4 container.
    let container_profile = EncodingContainerProfileBuilder::new()
        .name("container")
        .format(&gst::Caps::new_simple("video/quicktime", &[]))
        .add_profile(&(video_profile))
        .add_profile(&(audio_profile))
        .build()?;

    pipeline.set_render_settings(out_uri, &container_profile)?;

    pipeline.set_mode(ges::PipelineFlags::RENDER)?;

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().ok_or("No bus")?;
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}

pub fn preview(
    videos: &[Arc<Video>],
    phonems: &[Phonem],
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let yt_ids = videos
        .iter()
        .map(|v| v.id.clone())
        .collect::<Vec<YoutubeId>>();
    let id = PreviewId::from_project_sentence(&yt_ids, phonems);
    let p = id.path();
    if !p.exists() {
        render_phonems(videos, &phonems, &p, true)
    } else {
        Ok(())
    }
    .map(|()| p)
}

pub fn render(
    videos: &[Arc<Video>],
    phonems: &[Phonem],
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let yt_ids = videos
        .iter()
        .map(|v| v.id.clone())
        .collect::<Vec<YoutubeId>>();
    let id = PreviewId::from_project_sentence(&yt_ids, phonems);
    let p = id.render_path();
    if !p.exists() {
        render_phonems(videos, &phonems, &p, false)
    } else {
        Ok(())
    }
    .map(|()| p)
}

fn render_phonems(
    videos: &[Arc<Video>],
    phonems: &[Phonem],
    out_path: &std::path::Path,
    small: bool,
) -> BoxResult {
    ges::init()?; // TODO: peut etre l'enlever ?

    let uri = format!(
        "file://{}",
        out_path
            .to_str()
            .ok_or("Output path is not valid unicode")?
    );

    // Begin by creating a timeline with audio and video tracks
    let timeline = ges::Timeline::new();

    let video_caps = gst::Caps::new_simple("video/x-raw", &[]);
    let audio_track = ges::Track::new(
        ges::TrackType::AUDIO,
        &gst::Caps::new_simple("audio/x-raw", &[]),
    );
    let video_track = ges::Track::new(ges::TrackType::VIDEO, &video_caps);
    video_track.set_restriction_caps(&video_caps);

    timeline.add_track(&video_track)?;
    timeline.add_track(&audio_track)?;

    // Create a new layer that will contain our timed clips.
    let layer = timeline.append_layer();
    let pipeline = ges::Pipeline::new();
    pipeline.set_timeline(&timeline)?;

    let assets: Vec<_> = videos
        .iter()
        .map(|v| if small { &v.lite_asset } else { &v.asset })
        .collect(); // TODO once per project

    phonems.iter().try_fold(
        0,
        |timeline_start_ms: u64, e: &Phonem| -> Result<u64, Box<dyn std::error::Error>> {
            let start_ms = (e.start * 1000.0).round() as u64;
            let duration_ms = ((e.end - e.start) * 1000.0).round() as u64;
            let asset = assets[e.video_index as usize];
            // let clip = layer.add_asset(
            layer.add_asset(
                asset,
                ClockTime::from_mseconds(timeline_start_ms),
                ClockTime::from_mseconds(start_ms),
                ClockTime::from_mseconds(duration_ms),
                ges::TrackType::VIDEO | ges::TrackType::AUDIO,
            )?;
            // let effect = ges::Effect::new("head_tracking").expect("Failed to create effect");
            // clip.add(&effect)?;
            Ok(timeline_start_ms + duration_ms)
        },
    )?;

    render_pipeline(&pipeline, &uri, None)
}

pub fn render_main_video(
    in_path: &std::path::Path,
    out_path: &std::path::Path,
    small: bool,
) -> BoxResult {
    if out_path.exists() {
        return Ok(());
    }

    ges::init()?;

    let in_uri = format!(
        "file://{}",
        in_path.to_str().ok_or("Input path is not valid unicode")?
    );
    let out_uri = format!(
        "file://{}",
        out_path.to_str().ok_or("Input path is not valid unicode")?
    );

    // Begin by creating a timeline with audio and video tracks
    let timeline = ges::Timeline::new();

    let video_caps = gst::Caps::new_simple("video/x-raw", &[]);
    let audio_track = ges::Track::new(
        ges::TrackType::AUDIO,
        &gst::Caps::new_simple("audio/x-raw", &[]),
    );
    let video_track = ges::Track::new(ges::TrackType::VIDEO, &video_caps);
    video_track.set_restriction_caps(&video_caps);

    timeline.add_track(&video_track)?;
    timeline.add_track(&audio_track)?;

    // Create a new layer that will contain our timed clips.
    let layer = timeline.append_layer();
    let pipeline = ges::Pipeline::new();
    pipeline.set_timeline(&timeline)?;

    // TODO async
    let asset = ges::UriClipAsset::request_sync(&in_uri)?;

    let clip = layer.add_asset(
        &asset,
        ClockTime::from_mseconds(0),
        ClockTime::from_mseconds(0),
        asset.duration(),
        ges::TrackType::VIDEO | ges::TrackType::AUDIO,
    )?;
    if small {
        let effect = ges::Effect::new("videoscale").expect("Failed to create effect");
        clip.add(&effect)?;
        clip.set_child_property("width", &64i32.to_value())?;
        clip.set_child_property("height", &36i32.to_value())?;
        clip.set_child_property("method", &"nearest".to_value())?;
    }

    let caps = &gst::Caps::new_simple(
        "video/x-h264",
        if small {
            &[("width", &64i32), ("height", &36i32)]
        } else {
            &[]
        },
    );

    render_pipeline(&pipeline, &out_uri, Some(caps))
}
