import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { createLruCache, formatDuration } from "../utils";

type VideoThumb = {
  thumbnail: string | null;
  duration: number | null;
  error: boolean;
};

const videoThumbCache = createLruCache<string, VideoThumb>(50);
const videoThumbInFlight = new Map<string, Promise<VideoThumb>>();

/** Video thumbnail component - extracts a frame from the video */
export function VideoThumbnail({ filePath }: { filePath: string }) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [thumbnail, setThumbnail] = useState<string | null>(null);
  const [duration, setDuration] = useState<number | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    const cached = videoThumbCache.get(filePath);
    if (cached) {
      setThumbnail(cached.thumbnail);
      setDuration(cached.duration);
      setError(cached.error);
      return;
    }

    const existing = videoThumbInFlight.get(filePath);
    if (existing) {
      let cancelled = false;
      existing.then((result) => {
        if (cancelled) return;
        setThumbnail(result.thumbnail);
        setDuration(result.duration);
        setError(result.error);
      });
      return () => {
        cancelled = true;
      };
    }

    const video = videoRef.current;
    const canvas = canvasRef.current;
    if (!video || !canvas) return;

    let cancelled = false;
    let resolved = false;
    let resolvePromise: ((value: VideoThumb) => void) | null = null;
    const inFlightPromise = new Promise<VideoThumb>((resolve) => {
      resolvePromise = resolve;
    });
    videoThumbInFlight.set(filePath, inFlightPromise);

    const finalize = (result: VideoThumb, { cache }: { cache: boolean }) => {
      if (resolved) return;
      resolved = true;
      if (cache) {
        videoThumbCache.set(filePath, result);
      }
      videoThumbInFlight.delete(filePath);
      resolvePromise?.(result);
      if (!cancelled) {
        setThumbnail(result.thumbnail);
        setDuration(result.duration);
        setError(result.error);
      }
    };

    const handleLoadedMetadata = () => {
      if (cancelled) return;
      setDuration(video.duration);
      // Seek to 1 second or 10% of the video, whichever is smaller
      video.currentTime = Math.min(1, video.duration * 0.1);
    };

    const handleSeeked = () => {
      if (cancelled) return;
      try {
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        // Set canvas size to video dimensions (scaled down)
        const scale = Math.min(280 / video.videoWidth, 180 / video.videoHeight);
        canvas.width = video.videoWidth * scale;
        canvas.height = video.videoHeight * scale;

        ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
        finalize(
          { thumbnail: canvas.toDataURL("image/jpeg", 0.8), duration: video.duration, error: false },
          { cache: true }
        );
      } catch (e) {
        console.error("[powerpaste] Failed to capture video frame:", e);
        finalize({ thumbnail: null, duration: null, error: true }, { cache: true });
      }
    };

    const handleError = () => {
      if (cancelled) return;
      finalize({ thumbnail: null, duration: null, error: true }, { cache: true });
    };

    video.addEventListener("loadedmetadata", handleLoadedMetadata);
    video.addEventListener("seeked", handleSeeked);
    video.addEventListener("error", handleError);

    // Convert local file path to Tauri asset URL
    try {
      video.src = convertFileSrc(filePath);
      video.load();
    } catch (e) {
      console.error("[powerpaste] Failed to load video:", e);
      finalize({ thumbnail: null, duration: null, error: true }, { cache: true });
    }

    return () => {
      cancelled = true;
      video.removeEventListener("loadedmetadata", handleLoadedMetadata);
      video.removeEventListener("seeked", handleSeeked);
      video.removeEventListener("error", handleError);
      video.src = "";
      if (!resolved) {
        videoThumbInFlight.delete(filePath);
        resolvePromise?.({ thumbnail: null, duration: null, error: true });
      }
    };
  }, [filePath]);

  if (error) {
    return (
      <div className="previewVideoPlaceholder">
        <span className="previewVideoIcon">🎬</span>
        <span className="previewVideoError">Preview unavailable</span>
      </div>
    );
  }

  return (
    <div className="previewVideoContainer">
      {/* Hidden video element for thumbnail extraction */}
      <video ref={videoRef} style={{ display: "none" }} muted preload="metadata" />
      <canvas ref={canvasRef} style={{ display: "none" }} />

      {thumbnail ? (
        <div className="previewVideoThumb">
          <img src={thumbnail} alt="Video thumbnail" className="previewVideoImage" />
          {duration !== null && (
            <span className="previewVideoDuration">{formatDuration(duration)}</span>
          )}
          <span className="previewVideoPlayIcon">▶</span>
        </div>
      ) : (
        <div className="previewVideoPlaceholder">
          <span className="previewVideoIcon">🎬</span>
          <span className="previewVideoLoading">Loading...</span>
        </div>
      )}
    </div>
  );
}

/** Image thumbnail for local image files */
export function LocalImageThumbnail({ filePath }: { filePath: string }) {
  const [error, setError] = useState(false);
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    try {
      setSrc(convertFileSrc(filePath));
    } catch {
      setError(true);
    }
  }, [filePath]);

  if (error || !src) {
    return (
      <div className="previewImagePlaceholder">
        <span className="previewImageIcon">🖼️</span>
        <span className="previewImageError">Preview unavailable</span>
      </div>
    );
  }

  return (
    <img
      src={src}
      alt="Image preview"
      className="previewImage"
      onError={() => setError(true)}
    />
  );
}
