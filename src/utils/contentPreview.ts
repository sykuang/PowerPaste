type GitHubRepoInfo = { owner: string; repo: string };
type ImgurInfo = { type: "image" | "album" | "gallery"; id: string };

export function createLruCache<K, V>(limit: number) {
  const map = new Map<K, V>();
  return {
    get(key: K): V | undefined {
      if (!map.has(key)) return undefined;
      const value = map.get(key) as V;
      map.delete(key);
      map.set(key, value);
      return value;
    },
    has(key: K): boolean {
      return map.has(key);
    },
    set(key: K, value: V) {
      if (map.has(key)) map.delete(key);
      map.set(key, value);
      if (map.size > limit) {
        const oldestKey = map.keys().next().value as K;
        map.delete(oldestKey);
      }
    },
  };
}

export function isVideoFile(path: string): boolean {
  const videoExts = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv", "flv", "ogv", "3gp"];
  const ext = path.split(".").pop()?.toLowerCase() || "";
  return videoExts.includes(ext);
}

export function isImageFile(path: string): boolean {
  const imageExts = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "tiff", "heic", "heif"];
  const ext = path.split(".").pop()?.toLowerCase() || "";
  return imageExts.includes(ext);
}

export function extractDomain(url: string): string {
  try {
    const u = new URL(url);
    return u.hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

export function getFaviconUrl(url: string): string {
  try {
    const u = new URL(url);
    // Use Google's favicon service - request 64px for Retina sharpness
    return `https://www.google.com/s2/favicons?domain=${u.hostname}&sz=64`;
  } catch {
    return "";
  }
}

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

function getGitHubRepoInfo(url: string): GitHubRepoInfo | null {
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

function getImgurInfo(url: string): ImgurInfo | null {
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

export function getThumbnailUrl(url: string): string | null {
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

export function getFileIcon(ext: string): string {
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
