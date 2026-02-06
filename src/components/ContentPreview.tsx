import { useEffect, useState } from "react";
import { ClipboardItem, getImageData, checkFileExists } from "../api";
import {
  createLruCache,
  extractDomain,
  formatBytes,
  getFaviconUrl,
  getFileIcon,
  getThumbnailUrl,
  isImageFile,
  isVideoFile,
} from "../utils";
import { LocalImageThumbnail, VideoThumbnail } from "./ContentPreviewHelpers";

const fileExistsCache = createLruCache<string, boolean>(500);
const fileExistsInFlight = new Map<string, Promise<boolean>>();

const imageDataCache = createLruCache<string, string | null>(50);
const imageDataInFlight = new Map<string, Promise<string | null>>();

interface ContentPreviewProps {
  item: ClipboardItem;
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
    const cached = fileExistsCache.get(filePath);
    if (cached !== undefined) {
      setFileExists(cached);
      return;
    }

    let inFlight = fileExistsInFlight.get(filePath);
    if (!inFlight) {
      inFlight = checkFileExists(filePath)
        .then((exists) => {
          fileExistsCache.set(filePath, exists);
          return exists;
        })
        .catch(() => {
          fileExistsCache.set(filePath, false);
          return false;
        })
        .finally(() => {
          fileExistsInFlight.delete(filePath);
        });
      fileExistsInFlight.set(filePath, inFlight);
    }

    inFlight.then((exists) => {
      if (cancelled) return;
      setFileExists(exists);
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

    if (imageDataCache.has(item.id)) {
      setImageDataUrl(imageDataCache.get(item.id) ?? null);
      setImageLoading(false);
      return;
    }

    let inFlight = imageDataInFlight.get(item.id);
    if (!inFlight) {
      inFlight = getImageData(item.id)
        .then((dataUrl) => {
          imageDataCache.set(item.id, dataUrl ?? null);
          return dataUrl ?? null;
        })
        .catch((err) => {
          imageDataCache.set(item.id, null);
          throw err;
        })
        .finally(() => {
          imageDataInFlight.delete(item.id);
        });
      imageDataInFlight.set(item.id, inFlight);
    }

    inFlight
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
