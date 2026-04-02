import React, { useState, useRef, useEffect } from 'react';
import { api, listenAiLog } from '../../api/tauri';
import type { DeviceAiAnalysis } from '../../api/tauri';
import { useAppStore } from '../../store';

interface AiChatPanelProps {
  filePath: string;
  context?: string;
  messages: ChatMessage[];
  onMessagesChange: (messages: ChatMessage[]) => void;
}

export interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: string;
}

export const AiChatPanel: React.FC<AiChatPanelProps> = ({ filePath, context, messages, onMessagesChange }) => {
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const settings = useAppStore(s => s.settings);
  const globalAiLoading = useAppStore(s => s.aiLoading);
  const globalAiNodeId = useAppStore(s => s.aiNodeId);
  const aiLiveLog = useAppStore(s => s.aiLiveLog);
  const fileLabel = filePath.split(/[\\/]/).pop() || 'report';
  const chatTaskId = `device-chat::${fileLabel}`;
  const loading = globalAiLoading && globalAiNodeId === chatTaskId;

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(scrollToBottom, [messages]);

  const sendMessage = async () => {
    if (!input.trim() || globalAiLoading) return;
    const userMsg: ChatMessage = {
      role: 'user',
      content: input.trim(),
      timestamp: new Date().toLocaleTimeString(),
    };
    const nextMessages = [...messages, userMsg];
    onMessagesChange(nextMessages);
    setInput('');

    useAppStore.setState({
      aiLoading: true,
      aiNodeId: chatTaskId,
      aiLiveLog: [],
      aiError: null,
      error: null,
    });
    let unlistenLog: (() => void) | null = null;
    try {
      const cliName = settings?.ai_cli || 'claude';
      const model = settings?.ai_model || undefined;
      const thinking = settings?.ai_thinking || undefined;
      const history = nextMessages
        .slice(-6)
        .map(msg => `${msg.role === 'user' ? 'User' : msg.role === 'assistant' ? 'Assistant' : 'System'}: ${msg.content}`)
        .join('\n');

      unlistenLog = await listenAiLog((line) => {
        useAppStore.setState((state) => ({ aiLiveLog: [...state.aiLiveLog, line] }));
      });

      const result: DeviceAiAnalysis = await api.runAiDeviceChat(
        filePath,
        userMsg.content,
        context,
        history,
        cliName,
        model,
        thinking,
      );

      const aiMsg: ChatMessage = {
        role: 'assistant',
        content: result.analysis,
        timestamp: new Date().toLocaleTimeString(),
      };
      onMessagesChange([...nextMessages, aiMsg]);
    } catch (err) {
      const errMsg: ChatMessage = {
        role: 'system',
        content: `错误: ${String(err)}`,
        timestamp: new Date().toLocaleTimeString(),
      };
      onMessagesChange([...nextMessages, errMsg]);
      useAppStore.setState({ error: String(err) });
    } finally {
      unlistenLog?.();
      useAppStore.setState({ aiLoading: false, aiNodeId: null });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const quickActions = [
    { label: '整体分析', prompt: '请对这次设备性能测试进行全面的分析和优化建议' },
    { label: 'FPS优化', prompt: '分析FPS瓶颈并给出具体的优化方案' },
    { label: '内存泄漏', prompt: '检查是否存在内存泄漏风险，分析内存增长趋势' },
    { label: '热点函数', prompt: '分析CPU热点函数，找出最需要优化的函数调用' },
  ];

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      background: '#1a1a2e',
      borderLeft: '1px solid #333',
    }}>
      {/* Header */}
      <div style={{
        padding: '10px 12px',
        borderBottom: '1px solid #333',
        fontWeight: 600,
        fontSize: 13,
        color: '#4fc3f7',
        display: 'flex',
        alignItems: 'center',
        gap: 6,
      }}>
        🤖 AI 性能分析助手
      </div>

      {/* Quick Actions */}
      {messages.length === 0 && (
        <div style={{ padding: 12 }}>
          <div style={{ color: '#888', fontSize: 11, marginBottom: 8 }}>快速分析:</div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            {quickActions.map(qa => (
              <button
                key={qa.label}
                onClick={() => { setInput(qa.prompt); }}
                style={{
                  background: '#16213e',
                  color: '#4fc3f7',
                  border: '1px solid #0f3460',
                  borderRadius: 12,
                  padding: '4px 10px',
                  fontSize: 11,
                  cursor: 'pointer',
                }}
              >
                {qa.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Messages */}
      <div style={{
        flex: 1,
        overflowY: 'auto',
        padding: '8px 12px',
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
      }}>
        {messages.map((msg, i) => (
          <div key={i} style={{
            alignSelf: msg.role === 'user' ? 'flex-end' : 'flex-start',
            maxWidth: '90%',
            background: msg.role === 'user' ? '#0f3460' : msg.role === 'system' ? '#3d1212' : '#16213e',
            padding: '8px 12px',
            borderRadius: 8,
            fontSize: 12,
            color: msg.role === 'system' ? '#ef5350' : '#ddd',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
          }}>
            <div style={{ color: '#888', fontSize: 10, marginBottom: 4 }}>
              {msg.role === 'user' ? '你' : msg.role === 'assistant' ? 'AI' : '系统'} · {msg.timestamp}
            </div>
            {msg.content}
          </div>
        ))}
        {loading && (
          <div style={{
            alignSelf: 'stretch',
            background: '#101827',
            padding: '8px 12px',
            borderRadius: 8,
            fontSize: 12,
            color: '#9fb3c8',
            border: '1px solid #223247',
          }}>
            <div style={{ marginBottom: 6, color: '#4fc3f7', fontWeight: 600 }}>CLI 日志</div>
            <div style={{ maxHeight: 180, overflowY: 'auto', fontFamily: 'monospace', fontSize: 11, whiteSpace: 'pre-wrap', lineHeight: 1.5 }}>
              {aiLiveLog.length > 0 ? aiLiveLog.join('\n') : '正在等待 CLI 输出...'}
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div style={{
        padding: '8px 12px',
        borderTop: '1px solid #333',
        display: 'flex',
        gap: 6,
      }}>
        <textarea
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="输入分析问题... (Enter发送)"
          rows={2}
          style={{
            flex: 1,
            background: '#16213e',
            color: '#ccc',
            border: '1px solid #333',
            borderRadius: 6,
            padding: '6px 10px',
            fontSize: 12,
            resize: 'none',
            outline: 'none',
          }}
        />
        <button
          onClick={sendMessage}
          disabled={globalAiLoading || !input.trim()}
          style={{
            background: loading ? '#333' : '#0f3460',
            color: loading ? '#666' : '#4fc3f7',
            border: '1px solid #0f3460',
            borderRadius: 6,
            padding: '6px 12px',
            cursor: loading ? 'not-allowed' : 'pointer',
            fontSize: 12,
            alignSelf: 'flex-end',
          }}
        >
          发送
        </button>
      </div>
      {context && (
        <div style={{ padding: '4px 12px 8px', fontSize: 10, color: '#555' }}>
          上下文: {context.substring(0, 50)}...
        </div>
      )}
    </div>
  );
};

export default AiChatPanel;
