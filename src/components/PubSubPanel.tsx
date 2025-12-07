import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { subscribeChannel, publishMessage } from '../types/tauri';
import { Button } from './ui/Button';
import { Input } from './ui/Input';
import { useToast } from './ui/Toast';
import { Send, Radio, X } from 'lucide-react';
import { UnlistenFn } from '@tauri-apps/api/event';

interface PubSubPanelProps {
  connectionName: string;
}

interface Message {
  id: string;
  channel: string;
  content: string;
  timestamp: Date;
}

interface Subscription {
  channel: string;
  unlisten: UnlistenFn;
}

export function PubSubPanel({ connectionName }: PubSubPanelProps) {
  const { t } = useTranslation();
  const [activeSubscriptions, setActiveSubscriptions] = useState<Subscription[]>([]);
  const [messages, setMessages] = useState<Message[]>([]);
  const [newChannel, setNewChannel] = useState('');
  const [publishChannel, setPublishChannel] = useState('');
  const [publishMessageContent, setPublishMessageContent] = useState('');
  const { toast } = useToast();
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Cleanup subscriptions on unmount or connection change
  useEffect(() => {
    return () => {
      activeSubscriptions.forEach(sub => sub.unlisten());
    };
  }, [connectionName]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleSubscribe = async () => {
    if (!newChannel) return;
    if (activeSubscriptions.find(s => s.channel === newChannel)) {
      toast(t('pubsub.already_subscribed'), 'error');
      return;
    }

    try {
      const eventName = `redis:${connectionName}:${newChannel}`;
      const unlisten = await subscribeChannel(connectionName, newChannel, eventName, (msg) => {
        setMessages(prev => [...prev, {
          id: Math.random().toString(36),
          channel: newChannel,
          content: msg,
          timestamp: new Date()
        }]);
      });

      setActiveSubscriptions(prev => [...prev, { channel: newChannel, unlisten }]);
      setNewChannel('');
      if (!publishChannel) setPublishChannel(newChannel);
      toast(t('pubsub.subscribed_to', { channel: newChannel }), 'success');
    } catch (e: any) {
      toast(e.message || t('pubsub.subscribe_fail') || 'Failed to subscribe', 'error');
    }
  };

  const handleUnsubscribe = (channel: string) => {
    const sub = activeSubscriptions.find(s => s.channel === channel);
    if (sub) {
      sub.unlisten();
      setActiveSubscriptions(prev => prev.filter(s => s.channel !== channel));
      toast(t('pubsub.unsubscribed_from', { channel }), 'info');
    }
  };

  const handlePublish = async () => {
    if (!publishChannel || !publishMessageContent) return;
    try {
      const count = await publishMessage(connectionName, publishChannel, publishMessageContent);
      toast(t('pubsub.published', { count }), 'success');
      setPublishMessageContent('');
    } catch (e: any) {
      toast(e.message || t('pubsub.publish_fail'), 'error');
    }
  };

  return (
    <div className="flex h-full bg-slate-950 text-slate-100">
      {/* Sidebar - Subscriptions */}
      <div className="w-64 border-r border-slate-800 flex flex-col bg-slate-900/50">
        <div className="p-4 border-b border-slate-800">
          <h3 className="font-semibold mb-2 flex items-center gap-2">
            <Radio className="h-4 w-4 text-green-400" /> {t('pubsub.subscriptions')}
          </h3>
          <div className="flex gap-2">
            <Input 
              placeholder={t('pubsub.channel_placeholder')}
              value={newChannel}
              onChange={(e) => setNewChannel(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSubscribe()}
            />
            <Button size="sm" onClick={handleSubscribe}>{t('common.add')}</Button>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {activeSubscriptions.map(sub => (
            <div key={sub.channel} className="flex items-center justify-between p-2 rounded bg-slate-800/50 group">
              <span className="text-sm font-mono truncate" title={sub.channel}>{sub.channel}</span>
              <button onClick={() => handleUnsubscribe(sub.channel)} className="text-slate-500 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity">
                <X className="h-4 w-4" />
              </button>
            </div>
          ))}
          {activeSubscriptions.length === 0 && (
            <div className="text-center text-slate-500 text-sm py-4">
              {t('pubsub.no_subscriptions')}
            </div>
          )}
        </div>
      </div>

      {/* Main - Messages & Publish */}
      <div className="flex-1 flex flex-col">
        {/* Messages Log */}
        <div className="flex-1 overflow-y-auto p-4 space-y-2">
          {messages.length === 0 && (
            <div className="h-full flex items-center justify-center text-slate-600 flex-col gap-2">
               <Radio className="h-12 w-12 opacity-20" />
               <p>{t('pubsub.messages_placeholder')}</p>
            </div>
          )}
          {messages.map(msg => (
            <div key={msg.id} className="bg-slate-900 rounded p-3 border border-slate-800 animate-in slide-in-from-bottom-2">
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs font-bold text-blue-400 font-mono">{msg.channel}</span>
                <span className="text-xs text-slate-500">{msg.timestamp.toLocaleTimeString()}</span>
              </div>
              <div className="text-sm text-slate-200 break-all font-mono whitespace-pre-wrap">{msg.content}</div>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>

        {/* Publish Area */}
        <div className="p-4 border-t border-slate-800 bg-slate-900">
          <div className="flex gap-2 mb-2">
             <Input 
                placeholder={t('pubsub.target_channel')}
                className="w-48 font-mono"
                value={publishChannel}
                onChange={(e) => setPublishChannel(e.target.value)}
             />
          </div>
          <div className="flex gap-2">
            <textarea 
              className="flex-1 h-20 bg-slate-950 border border-slate-700 rounded-md p-3 text-sm font-mono focus:outline-none focus:ring-1 focus:ring-blue-500 resize-none"
              placeholder={t('pubsub.message_content')}
              value={publishMessageContent}
              onChange={(e) => setPublishMessageContent(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handlePublish();
                }
              }}
            />
            <Button className="h-20 w-20" onClick={handlePublish}>
              <Send className="h-6 w-6" />
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
