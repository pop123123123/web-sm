use crate::data::{Phonem, PreviewId, Video};

use ges::prelude::*;
use gst::ClockTime;
use gst_pbutils::{
    EncodingAudioProfileBuilder, EncodingContainerProfileBuilder, EncodingVideoProfileBuilder,
};

type BoxResult = Result<(), Box<dyn std::error::Error>>;

fn render_pipeline(pipeline: &ges::Pipeline, out_uri: &str) -> BoxResult {
    let audio_profile = EncodingAudioProfileBuilder::new()
        .format(&gst::Caps::new_simple("audio/x-vorbis", &[]))
        .presence(0)
        .build()?;

    // Every videostream piped into the encodebin should be encoded using theora.
    let video_profile = EncodingVideoProfileBuilder::new()
        .format(&gst::Caps::new_simple("video/x-theora", &[]))
        .presence(0)
        .build()?;

    // All streams are then finally combined into a matroska container.
    let container_profile = EncodingContainerProfileBuilder::new()
        .name("container")
        .format(&gst::Caps::new_simple("video/x-matroska", &[]))
        .add_profile(&(video_profile))
        .add_profile(&(audio_profile))
        .build()?;

    pipeline.set_render_settings(out_uri, &container_profile)?;

    pipeline.set_mode(ges::PipelineFlags::RENDER)?;

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().unwrap();
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

pub fn preview(videos: &[Video], phonems: &[Phonem]) -> BoxResult {
    let id = PreviewId::from_project_sentence(videos, phonems);
    let p = id.path();
    if !p.exists() {
        render_phonems(videos, &phonems, &p, true)
    } else {
        Ok(())
    }
}

pub fn render(videos: &[Video], phonems: &[Phonem]) -> BoxResult {
    let id = PreviewId::from_project_sentence(videos, phonems);
    let p = id.render_path();
    if !p.exists() {
        render_phonems(videos, &phonems, &p, false)
    } else {
        Ok(())
    }
}

fn render_phonems(
    videos: &[Video],
    phonems: &[Phonem],
    out_path: &std::path::Path,
    small: bool,
) -> BoxResult {
    ges::init()?;

    let uri = format!("file://{}", out_path.to_str().unwrap());

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
        .map(|v| {
            // TODO async
            ges::UriClipAsset::request_sync(&format!(
                "file://{}",
                (if small {
                    v.get_path_small()
                } else {
                    v.get_path_full_resolution()
                })
                .to_str()
                .unwrap()
            ))
            .unwrap()
        })
        .collect(); // TODO once per project

    phonems.iter().try_fold(
        0,
        |timeline_start_ms: u64, e: &Phonem| -> Result<u64, Box<dyn std::error::Error>> {
            let start_ms = (e.start * 1000.0).round() as u64;
            let duration_ms = ((e.end - e.start) * 1000.0).round() as u64;
            let asset = &assets[e.video_index as usize];
            // let clip = layer.add_asset(
            layer.add_asset(
                asset,
                ClockTime::from_mseconds(timeline_start_ms),
                ClockTime::from_mseconds(start_ms),
                ClockTime::from_mseconds(duration_ms),
                ges::TrackType::VIDEO | ges::TrackType::AUDIO,
            )?;
            // let effect = ges::Effect::new("head_tracking").expect("Failed to create effect");
            // clip.add(&effect).unwrap();
            Ok(timeline_start_ms + duration_ms)
        },
    )?;

    render_pipeline(&pipeline, &uri)
}
