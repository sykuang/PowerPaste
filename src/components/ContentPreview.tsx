import { useEffect, useRef, useState } from "react";
import { ClipboardItem, getImageData, checkFileExists } from "../api";
import { convertFileSrc } from "@tauri-apps/api/core";

interface ContentPreviewProps {
  item: ClipboardItem;
}

/** Format bytes to human readable string */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Format duration in seconds to mm:ss or hh:mm:ss */
function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** Check if a file path is a video file */
function isVideoFile(path: string): boolean {
  const videoExts = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv", "flv", "ogv", "3gp"];
  const ext = path.split(".").pop()?.toLowerCase() || "";
  return videoExts.includes(ext);
}

/** Check if a file path is an image file */
function isImageFile(path: string): boolean {
  const imageExts = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "tiff", "heic", "heif"];
  const ext = path.split(".").pop()?.toLowerCase() || "";
  return imageExts.includes(ext);
}

/** Extract domain from URL */
function extractDomain(url: string): string {
  try {
    const u = new URL(url);
    return u.hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** Get favicon URL for a domain */
function getFaviconUrl(url: string): string {
  try {
    const u = new URL(url);
    // Use Google's favicon service - request 64px for Retina sharpness
    return `https://www.google.com/s2/favicons?domain=${u.hostname}&sz=64`;
  } catch {
    return "";
  }
}

/** Extract YouTube video ID from URL */
function getYouTubeVideoId(url: string): string | null {
  try {
    const u = new URL(url);
    // Handle youtube.com/watch?v=VIDEO_ID
    if (u.hostname.includes("youtube.com")) {
      return u.searchParams.get("v");
    }
    // Handle youtu.be/VIDEO_ID
    if (u.hostname === "youtu.be") {
      return u.pathname.slice(1).split("/")[0];
    }
    return null;
  } catch {
    return null;
  }
}

/** Extract Vimeo video ID from URL */
function getVimeoVideoId(url: string): string | null {
  try {
    const u = new URL(url);
    if (u.hostname.includes("vimeo.com")) {
      // Handle vimeo.com/VIDEO_ID or vimeo.com/channels/.../VIDEO_ID
      const pathParts = u.pathname.split("/").filter(Boolean);
      // Last numeric segment is usually the video ID
      for (let i = pathParts.length - 1; i >= 0; i--) {
        if (/^\d+$/.test(pathParts[i])) {
          return pathParts[i];
        }
      }
    }
    return null;
  } catch {
    return null;
  }
}

/** Extract Dailymotion video ID from URL */
function getDailymotionVideoId(url: string): string | null {
  try {
    const u = new URL(url);
    if (u.hostname.includes("dailymotion.com") || u.hostname === "dai.ly") {
      // Handle dailymotion.com/video/VIDEO_ID or dai.ly/VIDEO_ID
      const match = u.pathname.match(/\/video\/([a-zA-Z0-9]+)/) || 
                    u.pathname.match(/^\/([a-zA-Z0-9]+)/);
      return match ? match[1] : null;
    }
    return null;
  } catch {
    return null;
  }
}

/** Extract GitHub repo info from URL */
function getGitHubRepoInfo(url: string): { owner: string; repo: string } | null {
  try {
    const u = new URL(url);
    if (!u.hostname.includes("github.com")) return null;
    
    const pathParts = u.pathname.split("/").filter(Boolean);
    if (pathParts.length >= 2) {
      return { owner: pathParts[0], repo: pathParts[1] };
    }
    return null;
  } catch {
    return null;
  }
}

/** Extract Imgur info from URL */
function getImgurInfo(url: string): { type: "image" | "album" | "gallery"; id: string } | null {
  try {
    const u = new URL(url);
    if (!u.hostname.includes("imgur.com")) return null;
    
    // Handle i.imgur.com/IMAGE_ID.ext
    if (u.hostname === "i.imgur.com") {
      const match = u.pathname.match(/\/([a-zA-Z0-9]+)/);
      if (match) return { type: "image", id: match[1] };
    }
    
    // Handle imgur.com/a/ALBUM_ID or imgur.com/gallery/ID
    const pathParts = u.pathname.split("/").filter(Boolean);
    if (pathParts[0] === "a" && pathParts[1]) {
      return { type: "album", id: pathParts[1] };
    }
    if (pathParts[0] === "gallery" && pathParts[1]) {
      return { type: "gallery", id: pathParts[1] };
    }
    // Handle imgur.com/IMAGE_ID
    if (pathParts[0] && !["a", "gallery", "t", "user", "r"].includes(pathParts[0])) {
      return { type: "image", id: pathParts[0] };
    }
    return null;
  } catch {
    return null;
  }
}

/** Extract Giphy info from URL */
function getGiphyId(url: string): string | null {
  try {
    const u = new URL(url);
    if (!u.hostname.includes("giphy.com")) return null;
    
    // Handle giphy.com/gifs/NAME-ID or media.giphy.com/media/ID/giphy.gif
    const match = u.pathname.match(/\/gifs\/.*-([a-zA-Z0-9]+)$/) ||
                  u.pathname.match(/\/media\/([a-zA-Z0-9]+)/);
    return match ? match[1] : null;
  } catch {
    return null;
  }
}

/** Get website thumbnail URL */
function getThumbnailUrl(url: string): string | null {
  try {
    const trimmedUrl = url.trim();
    
    // YouTube - official thumbnail API
    const youtubeId = getYouTubeVideoId(trimmedUrl);
    if (youtubeId) {
      return `https://img.youtube.com/vi/${youtubeId}/hqdefault.jpg`;
    }
    
    // Vimeo - use vumbnail.com (free, no auth required)
    const vimeoId = getVimeoVideoId(trimmedUrl);
    if (vimeoId) {
      return `https://vumbnail.com/${vimeoId}.jpg`;
    }
    
    // Dailymotion - official thumbnail API
    const dailymotionId = getDailymotionVideoId(trimmedUrl);
    if (dailymotionId) {
      return `https://www.dailymotion.com/thumbnail/video/${dailymotionId}`;
    }
    
    // GitHub - OpenGraph image (works without auth for public repos)
    const githubInfo = getGitHubRepoInfo(trimmedUrl);
    if (githubInfo) {
      return `https://opengraph.githubassets.com/1/${githubInfo.owner}/${githubInfo.repo}`;
    }
    
    // Imgur - direct image URL
    const imgurInfo = getImgurInfo(trimmedUrl);
    if (imgurInfo && imgurInfo.type === "image") {
      return `https://i.imgur.com/${imgurInfo.id}l.jpg`; // 'l' suffix = large thumbnail
    }
    
    // Giphy - direct GIF thumbnail
    const giphyId = getGiphyId(trimmedUrl);
    if (giphyId) {
      return `https://media.giphy.com/media/${giphyId}/giphy_s.gif`; // 's' = small still
    }
    
    // For other sites, use favicon-based fallback
    return null;
  } catch {
    return null;
  }
}

/** Video thumbnail component - extracts a frame from the video */
function VideoThumbnail({ filePath }: { filePath: string }) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [thumbnail, setThumbnail] = useState<string | null>(null);
  const [duration, setDuration] = useState<number | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    const video = videoRef.current;
    const canvas = canvasRef.current;
    if (!video || !canvas) return;

    let cancelled = false;

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
        setThumbnail(canvas.toDataURL("image/jpeg", 0.8));
      } catch (e) {
        console.error("[powerpaste] Failed to capture video frame:", e);
        setError(true);
      }
    };

    const handleError = () => {
      if (cancelled) return;
      setError(true);
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
      setError(true);
    }

    return () => {
      cancelled = true;
      video.removeEventListener("loadedmetadata", handleLoadedMetadata);
      video.removeEventListener("seeked", handleSeeked);
      video.removeEventListener("error", handleError);
      video.src = "";
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
function LocalImageThumbnail({ filePath }: { filePath: string }) {
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

export function ContentPreview({ item }: ContentPreviewProps) {
  const [imageDataUrl, setImageDataUrl] = useState<string | null>(null);
  const [imageLoading, setImageLoading] = useState(false);
  const [imageError, setImageError] = useState(false);
  const [fileExists, setFileExists] = useState<boolean | null>(null);

  // Check if this is a file path that we can preview
  const filePath = item.file_paths?.split("\n")[0] || (item.content_type === "file" ? item.text.trim() : null);
  const isVideo = filePath && isVideoFile(filePath);
  const isLocalImage = filePath && isImageFile(filePath);
  const isFileItem = item.content_type === "file" || item.kind === "file";

  // Check if file exists when it's a file item
  useEffect(() => {
    if (!isFileItem || !filePath) {
      setFileExists(null);
      return;
    }
    
    let cancelled = false;
    checkFileExists(filePath)
      .then((exists) => {
        if (cancelled) return;
        setFileExists(exists);
      })
      .catch(() => {
        if (cancelled) return;
        // On error, assume file doesn't exist
        setFileExists(false);
      });
    
    return () => {
      cancelled = true;
    };
  }, [isFileItem, filePath]);

  // Load image data for image items
  useEffect(() => {
    if (item.kind !== "image") return;
    
    let cancelled = false;
    setImageLoading(true);
    setImageError(false);
    
    getImageData(item.id)
      .then((dataUrl) => {
        if (cancelled) return;
        setImageDataUrl(dataUrl);
        setImageLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setImageError(true);
        setImageLoading(false);
      });
    
    return () => {
      cancelled = true;
    };
  }, [item.id, item.kind]);

  // If this looks like a file but file doesn't exist, treat as text
  if (isFileItem && fileExists === false) {
    return (
      <div className="contentPreview contentPreviewText">
        <div className="trayCardText">{item.text}</div>
      </div>
    );
  }

  // Still checking if file exists - show loading or text temporarily
  if (isFileItem && fileExists === null) {
    // Show text while checking (avoids flicker for non-existent files)
    return (
      <div className="contentPreview contentPreviewText">
        <div className="trayCardText">{item.text}</div>
      </div>
    );
  }

  // Video file preview
  if (isVideo && filePath && fileExists) {
    const fileName = filePath.split("/").pop() || filePath.split("\\").pop() || "Video";
    return (
      <div className="contentPreview contentPreviewVideo">
        <VideoThumbnail filePath={filePath} />
        <div className="previewVideoInfo">
          <div className="previewFileName" title={filePath}>{fileName}</div>
        </div>
      </div>
    );
  }

  // Local image file preview
  if (isLocalImage && filePath && fileExists) {
    const fileName = filePath.split("/").pop() || filePath.split("\\").pop() || "Image";
    return (
      <div className="contentPreview contentPreviewImage">
        <div className="previewImageContainer">
          <LocalImageThumbnail filePath={filePath} />
        </div>
        <div className="previewMeta">{fileName}</div>
      </div>
    );
  }

  // Image preview
  if (item.kind === "image") {
    const dimensions = item.image_width && item.image_height
      ? `${item.image_width}×${item.image_height}`
      : null;
    const size = item.image_size_bytes
      ? formatBytes(item.image_size_bytes)
      : null;

    return (
      <div className="contentPreview contentPreviewImage">
        <div className="previewImageContainer">
          {imageLoading && (
            <div className="previewImagePlaceholder">
              <span className="previewImageIcon">🖼️</span>
              <span className="previewImageLoading">Loading...</span>
            </div>
          )}
          {imageError && (
            <div className="previewImagePlaceholder">
              <span className="previewImageIcon">🖼️</span>
              <span className="previewImageError">Failed to load</span>
            </div>
          )}
          {imageDataUrl && !imageLoading && (
            <img
              src={imageDataUrl}
              alt="Clipboard image"
              className="previewImage"
              onError={() => setImageError(true)}
            />
          )}
        </div>
        <div className="previewMeta">
          {dimensions && <span>{dimensions}</span>}
          {dimensions && size && <span> • </span>}
          {size && <span>{size}</span>}
        </div>
      </div>
    );
  }

  // URL preview with website thumbnail
  if (item.content_type === "url") {
    const url = item.text.trim();
    const domain = extractDomain(url);
    const faviconUrl = getFaviconUrl(url);
    const thumbnailUrl = getThumbnailUrl(url);
    const hasThumbnail = thumbnailUrl !== null;

    // If we have a direct thumbnail (YouTube, etc.), show thumbnail layout
    if (hasThumbnail) {
      return (
        <div className="contentPreview contentPreviewUrl">
          <div className="previewThumbnailContainer">
            <img
              src={thumbnailUrl}
              alt={`Preview of ${domain}`}
              className="previewThumbnail"
              loading="lazy"
              onError={(e) => {
                // Hide thumbnail on error, show fallback
                (e.target as HTMLImageElement).style.display = "none";
                const fallback = (e.target as HTMLImageElement).nextElementSibling;
                if (fallback) (fallback as HTMLElement).style.display = "flex";
              }}
            />
            <div className="previewThumbnailFallback" style={{ display: "none" }}>
              <span className="previewThumbnailIcon">🔗</span>
            </div>
          </div>
          <div className="previewUrlInfo">
            <div className="previewUrlHeader">
              {faviconUrl && (
                <img
                  src={faviconUrl}
                  alt=""
                  className="previewFavicon"
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.display = "none";
                  }}
                />
              )}
              <span className="previewUrlDomain">{domain}</span>
            </div>
            <div className="previewUrlFull" title={url}>
              {url}
            </div>
          </div>
        </div>
      );
    }

    // Fallback: favicon + domain display for sites without direct thumbnail
    return (
      <div className="contentPreview contentPreviewUrlSimple">
        <div className="previewUrlSimpleIcon">
          {faviconUrl ? (
            <img
              src={faviconUrl}
              alt=""
              className="previewFaviconLarge"
              onError={(e) => {
                (e.target as HTMLImageElement).style.display = "none";
                const fallback = (e.target as HTMLImageElement).nextElementSibling;
                if (fallback) (fallback as HTMLElement).style.display = "block";
              }}
            />
          ) : null}
          <span className="previewUrlLinkIcon" style={{ display: faviconUrl ? "none" : "block" }}>🔗</span>
        </div>
        <div className="previewUrlSimpleInfo">
          <div className="previewUrlDomain">{domain}</div>
          <div className="previewUrlFull" title={url}>
            {url}
          </div>
        </div>
      </div>
    );
  }

  // File preview (only if file exists)
  if ((item.content_type === "file" || item.kind === "file") && fileExists) {
    const paths = (item.file_paths || item.text).split("\n").filter(Boolean);
    const firstPath = paths[0] || "";
    const fileName = firstPath.split("/").pop() || firstPath.split("\\").pop() || firstPath;
    const fileCount = paths.length;

    // Determine file icon based on extension
    const ext = fileName.split(".").pop()?.toLowerCase() || "";
    const fileIcon = getFileIcon(ext);

    return (
      <div className="contentPreview contentPreviewFile">
        <div className="previewFileIcon">{fileIcon}</div>
        <div className="previewFileInfo">
          <div className="previewFileName" title={firstPath}>
            {fileName}
          </div>
          {fileCount > 1 && (
            <div className="previewFileCount">+{fileCount - 1} more files</div>
          )}
        </div>
      </div>
    );
  }

  // Default text preview
  return (
    <div className="contentPreview contentPreviewText">
      <div className="trayCardText">{item.text}</div>
    </div>
  );
}

/** Get an emoji icon based on file extension */
function getFileIcon(ext: string): string {
  const imageExts = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico"];
  const videoExts = ["mp4", "mov", "avi", "mkv", "webm"];
  const audioExts = ["mp3", "wav", "aac", "flac", "ogg", "m4a"];
  const docExts = ["pdf", "doc", "docx", "txt", "rtf", "odt"];
  const codeExts = ["js", "ts", "jsx", "tsx", "py", "rs", "go", "java", "c", "cpp", "h", "css", "html", "json", "xml", "yaml", "yml", "md"];
  const archiveExts = ["zip", "rar", "7z", "tar", "gz", "bz2"];
  const spreadsheetExts = ["xls", "xlsx", "csv", "numbers"];
  const presentationExts = ["ppt", "pptx", "key"];

  if (imageExts.includes(ext)) return "🖼️";
  if (videoExts.includes(ext)) return "🎬";
  if (audioExts.includes(ext)) return "🎵";
  if (docExts.includes(ext)) return "📄";
  if (codeExts.includes(ext)) return "💻";
  if (archiveExts.includes(ext)) return "📦";
  if (spreadsheetExts.includes(ext)) return "📊";
  if (presentationExts.includes(ext)) return "📽️";
  
  return "📁";
}
