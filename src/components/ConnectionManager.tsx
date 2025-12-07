import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { listConfigs, addConnection, removeConnection, checkConnection, testConnectionConfig, ConfigItem, RedisConfig } from '../types/tauri';
import { Button } from './ui/Button';
import { Input } from './ui/Input';
import { Modal } from './ui/Modal';
import { useToast } from './ui/Toast';
import { Plus, Trash2, Activity, Database, Server, Edit, ChevronLeft, Settings } from 'lucide-react';
import { cn } from '../utils';

interface ConnectionManagerProps {
  selectedName: string | null;
  onSelect: (name: string) => void;
  onCollapse?: () => void;
}

export function ConnectionManager({ selectedName, onSelect, onCollapse }: ConnectionManagerProps) {
  const { t, i18n } = useTranslation();
  const [configs, setConfigs] = useState<ConfigItem[]>([]);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const { toast } = useToast();

  // Form state
  const [newName, setNewName] = useState('');
  const [mode, setMode] = useState<'standalone' | 'cluster' | 'sentinel'>('standalone');
  // Standalone
  const [newUrl, setNewUrl] = useState('redis://127.0.0.1:6379');
  // Cluster
  const [clusterNodes, setClusterNodes] = useState('redis://127.0.0.1:7000\nredis://127.0.0.1:7001');
  // Sentinel
  const [sentinelMaster, setSentinelMaster] = useState('mymaster');
  const [sentinelNodes, setSentinelNodes] = useState('redis://127.0.0.1:26379\nredis://127.0.0.1:26380');
  
  // Shared
  const [newPassword, setNewPassword] = useState('');

  const [deleteConfirmation, setDeleteConfirmation] = useState<string | null>(null);
  const [editingName, setEditingName] = useState<string | null>(null);

  // Error Modal state
  const [errorModalOpen, setErrorModalOpen] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');

  const loadConfigs = async () => {
    try {
      const items = await listConfigs();
      setConfigs(items);
    } catch (e) {
      console.error(e);
      toast('Failed to load configs', 'error');
    }
  };

  useEffect(() => {
    loadConfigs();
  }, []);

  const injectPassword = (url: string, password: string): string => {
    if (!password) return url;
    try {
      // Handle simple host:port case by adding redis://
      let urlToParse = url;
      if (!url.includes('://')) {
        urlToParse = `redis://${url}`;
      }
      
      const urlObj = new URL(urlToParse);
      if (!urlObj.password) {
        urlObj.password = password;
        if (!urlObj.username) {
          urlObj.username = ''; 
        }
      }
      return urlObj.toString();
    } catch (e) {
      // Fallback for partial URLs or other formats
      if (url.startsWith("redis://") && !url.includes("@")) {
         return url.replace("redis://", `redis://:${password}@`);
      }
      return url;
    }
  };

  const parseUrl = (url: string): { cleanUrl: string; password?: string } => {
    try {
      // Handle simple host:port case
      let urlToParse = url;
      if (!url.includes('://')) {
        urlToParse = `redis://${url}`;
      }
      
      const urlObj = new URL(urlToParse);
      const password = urlObj.password || undefined;
      
      // Remove auth info from URL object for clean display
      urlObj.password = '';
      urlObj.username = '';
      
      // If original didn't have protocol, remove it from result
      let cleanUrl = urlObj.toString();
      // Remove trailing slash if original didn't have it or path is empty
      if (cleanUrl.endsWith('/') && urlObj.pathname === '/') {
          cleanUrl = cleanUrl.slice(0, -1);
      }
      
      // Basic cleanup: if original was just host:port, try to return that style or keep redis://
      // But for consistency we can keep redis://
      
      return { cleanUrl, password };
    } catch (e) {
      // Fallback regex for redis://:password@host...
      const match = url.match(/redis:\/\/:(.*?)@(.*)/);
      if (match) {
        return { cleanUrl: `redis://${match[2]}`, password: match[1] };
      }
      return { cleanUrl: url };
    }
  };

  const handleEdit = async (item: ConfigItem, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingName(item.name);
    setNewName(item.name);
    
    const cfg = item.config;
    if (cfg.cluster) {
      setMode('cluster');
      // Extract password from first node if available
      let foundPassword = '';
      const nodes = cfg.urls.map(u => {
        const { cleanUrl, password } = parseUrl(u);
        if (password) foundPassword = password;
        return cleanUrl;
      });
      setClusterNodes(nodes.join('\n'));
      setNewPassword(foundPassword);
    } else if (cfg.sentinel) {
      setMode('sentinel');
      setSentinelMaster(cfg.sentinel_master_name || '');
      let foundPassword = '';
      const nodes = (cfg.sentinel_urls || []).map(u => {
        const { cleanUrl, password } = parseUrl(u);
        if (password) foundPassword = password;
        return cleanUrl;
      });
      setSentinelNodes(nodes.join('\n'));
      setNewPassword(foundPassword);
    } else {
      setMode('standalone');
      const { cleanUrl, password } = parseUrl(cfg.urls[0] || '');
      setNewUrl(cleanUrl);
      setNewPassword(password || '');
    }
    
    setIsModalOpen(true);
  };

  const handleSelect = async (name: string) => {
    try {
      await checkConnection(name);
      onSelect(name);
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : (e.message || JSON.stringify(e));
      setErrorMessage(`${t('connection.connect_failed')}: ${msg}`);
      setErrorModalOpen(true);
    }
  };

  const buildConfig = (): RedisConfig => {
    let config: RedisConfig;
    if (mode === 'standalone') {
      if (!newUrl) throw new Error(t('connection.required_url'));
      const finalUrl = injectPassword(newUrl, newPassword);
      config = { urls: [finalUrl] };
    } else if (mode === 'cluster') {
      const nodes = clusterNodes.split('\n').map(s => s.trim()).filter(Boolean);
      if (nodes.length === 0) throw new Error(t('connection.required_seed'));
      const finalNodes = nodes.map(url => injectPassword(url, newPassword));
      config = { 
        cluster: true,
        urls: finalNodes 
      };
    } else { // sentinel
      if (!sentinelMaster) throw new Error(t('connection.required_master'));
      const nodes = sentinelNodes.split('\n').map(s => s.trim()).filter(Boolean);
      if (nodes.length === 0) throw new Error(t('connection.required_sentinel'));
      const finalNodes = nodes.map(url => injectPassword(url, newPassword));
      config = {
        urls: [], 
        sentinel: true,
        sentinel_master_name: sentinelMaster,
        sentinel_urls: finalNodes
      };
    }
    return config;
  };

  const handleTestConnection = async () => {
    try {
      setLoading(true);
      const config = buildConfig();
      await testConnectionConfig(config);
      toast(t('connection.test_success'), 'success');
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : (e.message || JSON.stringify(e));
      setErrorMessage(msg);
      setErrorModalOpen(true);
    } finally {
      setLoading(false);
    }
  };

  const handleAdd = async () => {
    if (!newName) {
      toast(t('connection.required_name'), 'error');
      return;
    }
    const finalName = newName.trim();
    if (!finalName) {
      toast(t('connection.required_name'), 'error');
      return;
    }

    setLoading(true);
    try {
      const config = buildConfig();

      if (editingName && editingName !== finalName) {
        // Rename: add new first, then remove old
        await addConnection(finalName, config);
        await removeConnection(editingName);
        toast(t('connection.updated'), 'success');
      } else {
        // Add or Update same name
        await addConnection(finalName, config);
        toast(editingName ? t('connection.updated') : t('connection.added'), 'success');
      }

      setIsModalOpen(false);
      
      // Reset form
      setEditingName(null);
      setNewName('');
      setMode('standalone');
      setNewUrl('redis://127.0.0.1:6379');
      setNewPassword('');
      loadConfigs();
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : (e.message || JSON.stringify(e));
      toast(msg, 'error');
    } finally {
      setLoading(false);
    }
  };

  const openAddModal = () => {
    setEditingName(null);
    setNewName('');
    setMode('standalone');
    setNewUrl('redis://127.0.0.1:6379');
    setNewPassword('');
    setClusterNodes('redis://127.0.0.1:7000\nredis://127.0.0.1:7001');
    setSentinelMaster('mymaster');
    setSentinelNodes('redis://127.0.0.1:26379\nredis://127.0.0.1:26380');
    setIsModalOpen(true);
  };

  const handleRemove = async (name: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setDeleteConfirmation(name);
  };

  const confirmDelete = async () => {
    if (!deleteConfirmation) return;
    const name = deleteConfirmation;
    try {
      console.log(`Attempting to remove connection: "${name}"`);
      const res = await removeConnection(name);
      console.log(`Remove response: ${res}`);
      toast('Connection removed', 'success');
      if (selectedName === name) onSelect('');
      loadConfigs();
    } catch (e: any) {
      console.error("Remove failed:", e);
      toast(e.message || 'Failed to remove', 'error');
    } finally {
      setDeleteConfirmation(null);
    }
  };

  const handleCheck = async (name: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await checkConnection(name);
      toast(`${name} is healthy`, 'success');
    } catch (e: any) {
      toast(`${name} is unreachable: ${e.message}`, 'error');
    }
  };

  return (
    <div className="flex flex-col h-full bg-slate-900 border-r border-slate-800 w-64">
      <div className="p-4 border-b border-slate-800 flex items-center justify-between">
        <h2 className="font-semibold text-slate-100 flex items-center gap-2">
          <Database className="h-4 w-4" />
          {t('app.connections')}
        </h2>
        <div className="flex items-center gap-1">
          <Button size="icon" variant="ghost" onClick={() => setIsSettingsOpen(true)} title="Settings">
            <Settings className="h-4 w-4" />
          </Button>
          <Button size="icon" variant="ghost" onClick={openAddModal} title={t('connection.add_new')}>
            <Plus className="h-4 w-4" />
          </Button>
          {onCollapse && (
            <Button size="icon" variant="ghost" onClick={onCollapse} title="Collapse Sidebar">
              <ChevronLeft className="h-4 w-4" />
            </Button>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {configs.map((item) => (
          <div
            key={item.name}
            onClick={() => handleSelect(item.name)}
            className={cn(
              "group flex items-center justify-between p-3 rounded-md cursor-pointer transition-all hover:bg-slate-800",
              selectedName === item.name ? "bg-slate-800 border-l-2 border-blue-500" : "border-l-2 border-transparent"
            )}
          >
            <div className="flex items-center gap-3 overflow-hidden">
              <Server className={cn("h-4 w-4", selectedName === item.name ? "text-blue-500" : "text-slate-400")} />
              <div className="flex flex-col truncate">
                <span className={cn("text-sm font-medium truncate", selectedName === item.name ? "text-slate-100" : "text-slate-300")}>
                  {item.name}
                </span>
                <span className="text-xs text-slate-500 truncate">
                  {item.config.cluster ? 'Cluster' : item.config.sentinel ? 'Sentinel' : item.config.urls[0]}
                </span>
              </div>
            </div>
            
            <div className="flex items-center opacity-0 group-hover:opacity-100 transition-opacity">
              <Button 
                variant="ghost" 
                size="icon" 
                className="h-6 w-6" 
                title="Check Health"
                onClick={(e) => handleCheck(item.name, e)}
              >
                <Activity className="h-3 w-3" />
              </Button>
              <Button 
                variant="ghost" 
                size="icon" 
                className="h-6 w-6 text-blue-400 hover:text-blue-300" 
                title={t('common.edit')}
                onClick={(e) => handleEdit(item, e)}
              >
                <Edit className="h-3 w-3" />
              </Button>
              <Button 
                variant="ghost" 
                size="icon" 
                className="h-6 w-6 text-red-400 hover:text-red-300" 
                title={t('common.remove')}
                onClick={(e) => handleRemove(item.name, e)}
              >
                <Trash2 className="h-3 w-3" />
              </Button>
            </div>
          </div>
        ))}
        
        {configs.length === 0 && (
          <div className="text-center p-4 text-slate-500 text-sm">
            {t('connection.no_connections')}
          </div>
        )}
      </div>

      <Modal isOpen={!!deleteConfirmation} onClose={() => setDeleteConfirmation(null)} title={t('connection.remove_title')}>
        <div className="space-y-4">
          <p className="text-sm text-slate-300">
            {t('connection.remove_confirm', { name: deleteConfirmation })}
          </p>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setDeleteConfirmation(null)}>{t('common.cancel')}</Button>
            <Button onClick={confirmDelete} className="bg-red-600 hover:bg-red-500 text-white">
              {t('common.remove')}
            </Button>
          </div>
        </div>
      </Modal>

      <Modal isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} title="Settings">
        <div className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium text-slate-300">Language</label>
            <div className="flex gap-2">
              <Button 
                variant={i18n.language === 'en' ? 'primary' : 'ghost'} 
                onClick={() => i18n.changeLanguage('en')}
                className="flex-1"
              >
                English
              </Button>
              <Button 
                variant={i18n.language === 'zh' ? 'primary' : 'ghost'} 
                onClick={() => i18n.changeLanguage('zh')}
                className="flex-1"
              >
                中文
              </Button>
            </div>
          </div>
        </div>
      </Modal>

      <Modal isOpen={isModalOpen} onClose={() => setIsModalOpen(false)} title={editingName ? t('connection.edit_connection') : t('connection.add_new')}>
        <div className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium text-slate-300">{t('connection.name')}</label>
            <Input 
              value={newName} 
              onChange={(e) => setNewName(e.target.value)} 
              placeholder="e.g. Localhost" 
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-slate-300">{t('connection.mode')}</label>
            <div className="flex bg-slate-900 p-1 rounded-md border border-slate-700">
              {(['standalone', 'cluster', 'sentinel'] as const).map((m) => (
                <button
                  key={m}
                  onClick={() => setMode(m)}
                  className={cn(
                    "flex-1 text-xs font-medium py-1.5 px-2 rounded-sm capitalize transition-all",
                    mode === m 
                      ? "bg-blue-600 text-white shadow-sm" 
                      : "text-slate-400 hover:text-slate-200 hover:bg-slate-800"
                  )}
                >
                  {m}
                </button>
              ))}
            </div>
          </div>

          {mode === 'standalone' && (
            <div className="space-y-2">
              <label className="text-sm font-medium text-slate-300">{t('connection.url')}</label>
              <Input 
                value={newUrl} 
                onChange={(e) => setNewUrl(e.target.value)} 
                placeholder="redis://127.0.0.1:6379" 
              />
            </div>
          )}

          {mode === 'cluster' && (
            <div className="space-y-2">
              <label className="text-sm font-medium text-slate-300">{t('connection.seed_nodes')}</label>
              <textarea
                className="flex w-full rounded-md border border-slate-700 bg-slate-900 py-2 px-3 text-sm text-slate-100 shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-blue-500 min-h-[80px]"
                value={clusterNodes}
                onChange={(e) => setClusterNodes(e.target.value)}
                placeholder="redis://127.0.0.1:7000&#10;redis://127.0.0.1:7001"
              />
            </div>
          )}

          {mode === 'sentinel' && (
            <>
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('connection.master_name')}</label>
                <Input 
                  value={sentinelMaster} 
                  onChange={(e) => setSentinelMaster(e.target.value)} 
                  placeholder="mymaster" 
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('connection.sentinel_nodes')}</label>
                <textarea
                  className="flex w-full rounded-md border border-slate-700 bg-slate-900 py-2 px-3 text-sm text-slate-100 shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-blue-500 min-h-[80px]"
                  value={sentinelNodes}
                  onChange={(e) => setSentinelNodes(e.target.value)}
                  placeholder="redis://127.0.0.1:26379&#10;redis://127.0.0.1:26380"
                />
              </div>
            </>
          )}

          <div className="space-y-2">
            <label className="text-sm font-medium text-slate-300">{t('connection.password')}</label>
            <Input 
              type="password"
              value={newPassword} 
              onChange={(e) => setNewPassword(e.target.value)} 
              placeholder="••••••" 
            />
            <p className="text-xs text-slate-500">
              {t('connection.password_hint')}
            </p>
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <div className="flex-1">
              <Button variant="outline" onClick={handleTestConnection} disabled={loading} type="button">
                {t('connection.test_connection')}
              </Button>
            </div>
            <Button variant="ghost" onClick={() => setIsModalOpen(false)}>{t('common.cancel')}</Button>
            <Button onClick={handleAdd} disabled={loading}>
              {loading ? t('connection.saving') : (editingName ? t('connection.save_changes') : t('connection.add_new'))}
            </Button>
          </div>
        </div>
      </Modal>

      <Modal isOpen={errorModalOpen} onClose={() => setErrorModalOpen(false)} title={t('connection.test_failed')}>
        <div className="space-y-4">
          <div className="text-sm text-red-400 bg-red-900/20 p-3 rounded-md border border-red-900/50 break-words font-mono">
            {errorMessage}
          </div>
          <div className="flex justify-end">
            <Button onClick={() => setErrorModalOpen(false)}>
              {t('common.confirm')}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
