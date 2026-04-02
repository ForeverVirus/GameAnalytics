import React, { useEffect, useMemo, useState } from 'react';
import { api } from '../../api/tauri';

interface ScreenshotViewerProps {
  filePath: string;
  totalFrames: number;
  availableFrames?: number[];
  /** Current frame index to show */
  currentFrame?: number;
  onFrameChange?: (frame: number) => void;
}

export const ScreenshotViewer: React.FC<ScreenshotViewerProps> = ({
  filePath,
  totalFrames,
  availableFrames,
  currentFrame = 0,
  onFrameChange,
}) => {
  const frameList = useMemo(() => {
    if (availableFrames && availableFrames.length > 0) {
      return Array.from(new Set(availableFrames)).sort((a, b) => a - b);
    }
    return Array.from({ length: totalFrames }, (_, index) => index);
  }, [availableFrames, totalFrames]);

  const resolveCursor = (frame: number) => {
    if (frameList.length === 0) return 0;
    const exactIndex = frameList.indexOf(frame);
    if (exactIndex >= 0) return exactIndex;

    let bestIndex = 0;
    let bestDistance = Infinity;
    frameList.forEach((candidate, index) => {
      const distance = Math.abs(candidate - frame);
      if (distance < bestDistance) {
        bestDistance = distance;
        bestIndex = index;
      }
    });
    return bestIndex;
  };

  const [cursor, setCursor] = useState(resolveCursor(currentFrame));
  const [imageData, setImageData] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const frame = frameList[cursor] ?? 0;

  useEffect(() => {
    setCursor(resolveCursor(currentFrame));
  }, [currentFrame, frameList]);

  useEffect(() => {
    if (frameList.length === 0) {
      setImageData(null);
      setError('无截图数据');
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    api.getDeviceScreenshot(filePath, frame)
      .then(data => {
        if (!cancelled) {
          setImageData(data);
          setLoading(false);
        }
      })
      .catch(err => {
        if (!cancelled) {
          setError(String(err));
          setImageData(null);
          setLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [filePath, frame, frameList.length]);

  const handleSlider = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextCursor = parseInt(e.target.value, 10);
    setCursor(nextCursor);
    onFrameChange?.(frameList[nextCursor] ?? 0);
  };

  const step = (delta: number) => {
    const nextCursor = Math.max(0, Math.min(frameList.length - 1, cursor + delta));
    setCursor(nextCursor);
    onFrameChange?.(frameList[nextCursor] ?? 0);
  };

  const jumpTo = (nextCursor: number) => {
    const clamped = Math.max(0, Math.min(frameList.length - 1, nextCursor));
    setCursor(clamped);
    onFrameChange?.(frameList[clamped] ?? 0);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{
        width: '100%',
        aspectRatio: '16/9',
        background: '#111',
        borderRadius: 4,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'hidden',
        border: '1px solid #333',
      }}>
        {loading && <span style={{ color: '#666' }}>加载截图...</span>}
        {error && <span style={{ color: '#666', fontSize: 12 }}>无截图数据</span>}
        {imageData && !loading && (
          <img
            src={`data:image/jpeg;base64,${imageData}`}
            alt={`Frame ${frame}`}
            style={{ maxWidth: '100%', maxHeight: '100%', objectFit: 'contain' }}
          />
        )}
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <button onClick={() => jumpTo(0)} style={btnStyle} title="第一张">⏮</button>
        <button onClick={() => step(-1)} style={btnStyle} title="上一张">◀</button>
        <input
          type="range"
          min={0}
          max={Math.max(0, frameList.length - 1)}
          value={cursor}
          onChange={handleSlider}
          style={{ flex: 1 }}
          disabled={frameList.length === 0}
        />
        <button onClick={() => step(1)} style={btnStyle} title="下一张">▶</button>
        <button onClick={() => jumpTo(frameList.length - 1)} style={btnStyle} title="最后一张">⏭</button>
        <span style={{ color: '#aaa', fontSize: 12, minWidth: 80, textAlign: 'right' }}>
          {frameList.length > 0 ? `第 ${cursor + 1} / ${frameList.length} 张` : '无截图'}
        </span>
      </div>
      {frameList.length > 0 && (
        <div style={{ fontSize: 12, color: '#888' }}>
          当前截图帧号: <span style={{ color: '#ccc' }}>#{frame}</span>
        </div>
      )}
    </div>
  );
};

const btnStyle: React.CSSProperties = {
  background: '#333',
  color: '#ccc',
  border: '1px solid #555',
  borderRadius: 3,
  padding: '2px 8px',
  cursor: 'pointer',
  fontSize: 12,
};

export default ScreenshotViewer;
