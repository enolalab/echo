export interface YtDlpStatus {
  installed: boolean;
  path?: string | null;
  version?: string | null;
}

export interface YoutubeVideoItem {
  id: string;
  title: string;
  duration_seconds?: number | null;
  thumbnail_url?: string | null;
  channel_name?: string | null;
  webpage_url: string;
}

export interface YoutubeInspectResult {
  is_playlist: boolean;
  playlist_id?: string | null;
  title: string;
  items: YoutubeVideoItem[];
}

export interface YoutubeDownloadRequest {
  items: YoutubeVideoItem[];
  output_dir?: string | null;
}

export interface YoutubeProgressPayload {
  video_id: string;
  index: number;
  total_items: number;
  stage: 'downloading' | 'merging' | 'completed' | 'error';
  percentage: number;
  speed?: string | null;
  eta?: string | null;
  saved_path?: string | null;
  error?: string | null;
}
