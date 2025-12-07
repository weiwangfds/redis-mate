import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { scanKeys, delKey, getValue, setValue, ttlKey, expireKey, persistKey, getDbSize, getKeyType, hgetAll, hset, hdel, lpush, rpop, lrange, sadd, smembers, srem, zadd, zrem, zrangeWithScores, jsonGet, jsonSet } from '../types/tauri';
import { Button } from './ui/Button';
import { Input } from './ui/Input';
import { useToast } from './ui/Toast';
import { RefreshCw, Search, Trash2, Clock, Plus, Save, Database, ChevronDown, ChevronRight, HelpCircle, Wand2, Copy } from 'lucide-react';
import { ChevronLeft, ChevronRight as ChevronRightIcon } from 'lucide-react';
import { Modal } from './ui/Modal';

interface KeyManagerProps {
  connectionName: string;
}

interface KeyDetail {
  key: string;
  value: string | null;
  ttl: number;
}

const isJsonString = (str: string) => {
  try {
    const o = JSON.parse(str);
    if (o && typeof o === "object") {
      return true;
    }
  } catch (e) { }
  return false;
};

export function KeyManager({ connectionName }: KeyManagerProps) {
  const [keys, setKeys] = useState<string[]>([]);
  const [cursor, setCursor] = useState(0);
  const [pattern, setPattern] = useState('*');
  const [db, setDb] = useState(0);
  const [dbSize, setDbSize] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [keyDetail, setKeyDetail] = useState<KeyDetail | null>(null);
  const [keyType, setKeyType] = useState<string | null>(null);
  const [hashData, setHashData] = useState<Record<string, string>>({});
  const [listData, setListData] = useState<string[]>([]);
  const [setData, setSetData] = useState<string[]>([]);
  const [zsetData, setZsetData] = useState<[string, number][]>([]);
  const [newZsetMember, setNewZsetMember] = useState('');
  const [newZsetScore, setNewZsetScore] = useState('');
  const [jsonText, setJsonText] = useState('');
  const [stringFormatEnabled, setStringFormatEnabled] = useState(false);
  const [stringFormattedText, setStringFormattedText] = useState('');
  const [stringFormattedObject, setStringFormattedObject] = useState<any | null>(null);
  const [jsonCollapsed, setJsonCollapsed] = useState<Record<string, boolean>>({});
  const [newHashField, setNewHashField] = useState('');
  const [newHashValue, setNewHashValue] = useState('');
  const [newListValue, setNewListValue] = useState('');
  const [newSetMember, setNewSetMember] = useState('');
  const [detailLoading, setDetailLoading] = useState(false);
  const { toast } = useToast();
  const { t } = useTranslation();

  // Add Key Modal State
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [newKeyValue, setNewKeyValue] = useState('');
  const [newKeyTtl, setNewKeyTtl] = useState('');
  const [newKeyType, setNewKeyType] = useState<'string' | 'hash' | 'list' | 'set' | 'zset' | 'json'>('string');
  const [addHashField, setAddHashField] = useState('');
  const [addHashValue, setAddHashValue] = useState('');
  const [addListValue, setAddListValue] = useState('');
  const [addSetMember, setAddSetMember] = useState('');
  const [addZsetMember, setAddZsetMember] = useState('');
  const [addZsetScore, setAddZsetScore] = useState('');
  const [addJsonText, setAddJsonText] = useState('');

  const [keyTypes, setKeyTypes] = useState<Record<string, string>>({});
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});
  const [viewMode, setViewMode] = useState<'type' | 'tree'>('type');
  const [collapsedTree, setCollapsedTree] = useState<Record<string, boolean>>({});
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [autoIntervalMs, setAutoIntervalMs] = useState(5000);
  const [recentPath, setRecentPath] = useState<string | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const canFormatJson = useMemo(() => {
    return keyDetail?.value ? isJsonString(keyDetail.value) : false;
  }, [keyDetail?.value]);

  const ensureExpandedByPath = useCallback((path: string) => {
    const parts = path.split('/');
    setCollapsedTree(prev => {
      const n = { ...prev };
      let acc = '';
      for (let i = 0; i < parts.length; i++) {
        acc = i === 0 ? parts[i] : `${acc}/${parts[i]}`;
        n[acc] = false;
      }
      return n;
    });
  }, []);

  const storageKey = useMemo(() => `keymgr:${connectionName}:${db}`, [connectionName, db]);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(storageKey);
      if (raw) {
        const s = JSON.parse(raw);
        if (s && typeof s === 'object') {
          if (s.collapsedGroups) setCollapsedGroups(s.collapsedGroups);
          if (s.collapsedTree) setCollapsedTree(s.collapsedTree);
          if (s.recentPath) setRecentPath(s.recentPath);
          if (s.viewMode) setViewMode(s.viewMode);
          if (typeof s.autoRefresh === 'boolean') setAutoRefresh(s.autoRefresh);
          if (typeof s.autoIntervalMs === 'number') setAutoIntervalMs(s.autoIntervalMs);
          if (typeof s.sidebarCollapsed === 'boolean') setSidebarCollapsed(s.sidebarCollapsed);
          if (s.viewMode === 'tree' && s.recentPath) ensureExpandedByPath(s.recentPath);
        }
      }
    } catch {}
  }, [storageKey, ensureExpandedByPath]);

  useEffect(() => {
    try {
      const s = { collapsedGroups, collapsedTree, recentPath, viewMode, autoRefresh, autoIntervalMs, sidebarCollapsed };
      localStorage.setItem(storageKey, JSON.stringify(s));
    } catch {}
  }, [collapsedGroups, collapsedTree, recentPath, viewMode, autoRefresh, autoIntervalMs, sidebarCollapsed, storageKey]);

  const loadDbSize = useCallback(async () => {
    if (!connectionName) return;
    try {
      const size = await getDbSize(connectionName, db);
      setDbSize(size);
    } catch (e: any) {
      // Ignore error for cluster mode or if command fails, just don't show size
      console.error("Failed to get db size:", e);
      setDbSize(null);
    }
  }, [connectionName, db]);

  const loadKeys = useCallback(async (reset = false) => {
    if (!connectionName) return;
    setLoading(true);
    try {
      const currentCursor = reset ? 0 : cursor;
      const [nextCursor, newKeys] = await scanKeys(connectionName, db, currentCursor, pattern, 100);
      
      setKeys(prev => reset ? newKeys : [...prev, ...newKeys]);
      const types = await Promise.all(
        newKeys.map(k => getKeyType(connectionName, k, db).catch(() => 'unknown'))
      );
      setKeyTypes(prev => {
        const n = reset ? {} : { ...prev };
        newKeys.forEach((k, i) => { n[k] = types[i]; });
        return n;
      });
      setCursor(nextCursor);
      if (reset) {
        loadDbSize();
      }
    } catch (e: any) {
      toast(e.message || t('key_manager.scan_fail'), 'error');
    } finally {
      setLoading(false);
    }
  }, [connectionName, cursor, pattern, db, toast, loadDbSize]);

  // Reset and load when connection, pattern or db changes
  useEffect(() => {
    setCursor(0);
    setKeys([]);
    if (connectionName) {
      loadKeys(true);
    }
  }, [connectionName, pattern, db]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(() => {
      if (!loading && connectionName) {
        loadKeys(true);
      }
    }, autoIntervalMs);
    return () => clearInterval(id);
  }, [autoRefresh, autoIntervalMs, loading, connectionName, loadKeys]);

  useEffect(() => {
    if (viewMode !== 'tree') return;
    if (!recentPath) return;
    ensureExpandedByPath(recentPath);
  }, [keys, recentPath, viewMode, ensureExpandedByPath]);

  const loadKeyDetail = async (key: string) => {
    if (!connectionName) return;
    setDetailLoading(true);
    try {
      const [t, ttl] = await Promise.all([
        getKeyType(connectionName, key, db),
        ttlKey(connectionName, key, db)
      ]);
      setKeyType(t);
      setSelectedKey(key);
      if (t === 'string') {
        const value = await getValue(connectionName, key, db);
        setKeyDetail({ key, value, ttl });
        setHashData({});
        setListData([]);
        setSetData([]);
        
        // Auto detect JSON
        if (value && isJsonString(value)) {
            try {
                const obj = JSON.parse(value);
                const pretty = JSON.stringify(obj, null, 2);
                setStringFormattedText(pretty);
                setStringFormattedObject(obj);
                setStringFormatEnabled(true);
            } catch {}
        } else {
            setStringFormatEnabled(false);
            setStringFormattedText('');
            setStringFormattedObject(null);
        }
      } else if (t === 'hash') {
        const m = await hgetAll(connectionName, key, db);
        setKeyDetail({ key, value: null, ttl });
        setHashData(m);
        setListData([]);
        setSetData([]);
      } else if (t === 'list') {
        const items = await lrange(connectionName, key, 0, -1, db);
        setKeyDetail({ key, value: null, ttl });
        setListData(items);
        setHashData({});
        setSetData([]);
      } else if (t === 'set') {
        const members = await smembers(connectionName, key, db);
        setKeyDetail({ key, value: null, ttl });
        setSetData(members);
        setHashData({});
        setListData([]);
        setZsetData([]);
        setJsonText('');
      } else if (t === 'zset') {
        const pairs = await zrangeWithScores(connectionName, key, 0, -1, db);
        setKeyDetail({ key, value: null, ttl });
        setZsetData(pairs);
        setHashData({});
        setListData([]);
        setSetData([]);
        setJsonText('');
      } else if (t === 'stream') {
        setKeyDetail({ key, value: null, ttl });
        setHashData({});
        setListData([]);
        setSetData([]);
        setZsetData([]);
        setJsonText('');
      } else if (t === 'none') {
        setKeyDetail({ key, value: null, ttl });
        setHashData({});
        setListData([]);
        setSetData([]);
        setZsetData([]);
        setJsonText('');
      } else {
        const j = await jsonGet(connectionName, key, '$', db);
        setKeyDetail({ key, value: null, ttl });
        setJsonText(j ? JSON.stringify(j, null, 2) : '');
        setHashData({});
        setListData([]);
        setSetData([]);
        setZsetData([]);
      }
    } catch (e: any) {
      toast(e.message || t('key_manager.load_detail_fail'), 'error');
    } finally {
      setDetailLoading(false);
    }
  };

  const handleDelete = async (key: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    if (!confirm(t('key_manager.delete_confirm', { key }))) return;
    
    try {
      await delKey(connectionName, key, db);
      toast(t('key_manager.delete_success'), 'success');
      setKeys(prev => prev.filter(k => k !== key));
      setKeyTypes(prev => {
        const n = { ...prev };
        delete n[key];
        return n;
      });
      if (selectedKey === key) {
        setSelectedKey(null);
        setKeyDetail(null);
      }
      loadDbSize();
    } catch (e: any) {
      toast(e.message || t('key_manager.delete_fail'), 'error');
    }
  };

  const handleAddKey = async () => {
    if (!newKeyName) {
      toast(t('connection.required_name'), 'error');
      return;
    }
    try {
      const ttl = newKeyTtl ? parseInt(newKeyTtl) : undefined;
      if (newKeyType === 'string') {
        if (!newKeyValue) { toast(t('key_manager.value_required'), 'error'); return; }
        await setValue(connectionName, newKeyName, newKeyValue, ttl, db);
      } else if (newKeyType === 'hash') {
        if (!addHashField) { toast(t('key_manager.field_required'), 'error'); return; }
        await hset(connectionName, newKeyName, addHashField, addHashValue, db);
        if (ttl && ttl > 0) await expireKey(connectionName, newKeyName, ttl, db);
      } else if (newKeyType === 'list') {
        if (!addListValue) { toast(t('key_manager.value_required'), 'error'); return; }
        await lpush(connectionName, newKeyName, addListValue, db);
        if (ttl && ttl > 0) await expireKey(connectionName, newKeyName, ttl, db);
      } else if (newKeyType === 'set') {
        if (!addSetMember) { toast(t('key_manager.member_required'), 'error'); return; }
        await sadd(connectionName, newKeyName, addSetMember, db);
        if (ttl && ttl > 0) await expireKey(connectionName, newKeyName, ttl, db);
      } else if (newKeyType === 'zset') {
        if (!addZsetMember) { toast(t('key_manager.member_required'), 'error'); return; }
        const score = parseFloat(addZsetScore || '0');
        await zadd(connectionName, newKeyName, addZsetMember, score, db);
        if (ttl && ttl > 0) await expireKey(connectionName, newKeyName, ttl, db);
      } else if (newKeyType === 'json') {
        const parsed = addJsonText ? JSON.parse(addJsonText) : {};
        await jsonSet(connectionName, newKeyName, parsed, '$', db);
        if (ttl && ttl > 0) await expireKey(connectionName, newKeyName, ttl, db);
      }
      toast(t('key_manager.add_success'), 'success');
      setIsAddModalOpen(false);
      setNewKeyName('');
      setNewKeyValue('');
      setNewKeyTtl('');
      setNewKeyType('string');
      setAddHashField('');
      setAddHashValue('');
      setAddListValue('');
      setAddSetMember('');
      setAddZsetMember('');
      setAddZsetScore('');
      setAddJsonText('');
      loadKeys(true);
    } catch (e: any) {
      toast(e.message || t('key_manager.add_fail'), 'error');
    }
  };

  const handleUpdateValue = async () => {
    if (!keyDetail) return;
    try {
      if (keyType === 'string') {
        await setValue(connectionName, keyDetail.key, keyDetail.value || '', undefined, db);
      }
      toast(t('key_manager.value_updated'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.update_fail'), 'error');
    }
  };

  const toggleStringFormat = () => {
    if (!keyDetail) return;
    if (!stringFormatEnabled) {
      try {
        const v = keyDetail.value || '';
        const obj = JSON.parse(v);
        const pretty = JSON.stringify(obj, null, 2);
        setStringFormattedText(pretty);
        setStringFormattedObject(obj);
        setStringFormatEnabled(true);
      } catch (e: any) {
        toast(t('key_manager.invalid_json'), 'error');
      }
    } else {
      setStringFormatEnabled(false);
      setStringFormattedText('');
      setStringFormattedObject(null);
      setJsonCollapsed({});
    }
  };

  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast(t('common.copied'), 'success');
    } catch (e: any) {
      toast(t('key_manager.copy_fail'), 'error');
    }
  };

  const refreshStringFormat = () => {
    if (!keyDetail || !stringFormatEnabled) return;
    try {
      const v = keyDetail.value || '';
      const obj = JSON.parse(v);
      const pretty = JSON.stringify(obj, null, 2);
      setStringFormattedText(pretty);
      setStringFormattedObject(obj);
    } catch (e: any) {
      toast(t('key_manager.invalid_json'), 'error');
    }
  };

  const escapeHtml = (s: string) => s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');

  const mixWithWhite = (hex: string, factor: number) => {
    const h = hex.replace('#', '');
    const r = parseInt(h.substring(0, 2), 16);
    const g = parseInt(h.substring(2, 4), 16);
    const b = parseInt(h.substring(4, 6), 16);
    const rn = Math.round(r + (255 - r) * factor);
    const gn = Math.round(g + (255 - g) * factor);
    const bn = Math.round(b + (255 - b) * factor);
    const toHex = (n: number) => n.toString(16).padStart(2, '0');
    return `#${toHex(rn)}${toHex(gn)}${toHex(bn)}`;
  };

  const COLORS = {
    bracket: '#94a3b8',
    punctuation: '#06b6d4',
    key: '#60a5fa',
    string: '#10b981',
    number: '#f59e0b',
    boolean: '#8b5cf6',
    null: '#ec4899',
    index: '#6366f1',
  } as const;

  const tone = (hex: string, indent: number) => {
    const factor = Math.min(indent * 0.08, 0.6);
    const c = mixWithWhite(hex, factor);
    return `style=\"color:${c}\"`;
  };

  const color = (hex: string, indent: number) => {
    const factor = Math.min(indent * 0.08, 0.6);
    return mixWithWhite(hex, factor);
  };

  const renderJsonHtml = (value: any, indent = 0): string => {
    const pad = (n: number) => '&nbsp;'.repeat(n * 2);
    if (value === null) {
      return `<span ${tone(COLORS.null, indent)}>null</span>`;
    }
    if (typeof value === 'boolean') {
      return `<span ${tone(COLORS.boolean, indent)}>${value}</span>`;
    }
    if (typeof value === 'number') {
      return `<span ${tone(COLORS.number, indent)}>${value}</span>`;
    }
    if (typeof value === 'string') {
      return `<span ${tone(COLORS.string, indent)}>\"${escapeHtml(value)}\"</span>`;
    }
    if (Array.isArray(value)) {
      if (value.length === 0) return `<span ${tone(COLORS.bracket, indent)} class="font-semibold">[ ]</span>`;
      let out = `<span ${tone(COLORS.bracket, indent)} class="font-semibold">[</span><br/>`;
      for (let i = 0; i < value.length; i++) {
        out += `${pad(indent + 1)}<span ${tone(COLORS.index, indent + 1)}>[${i}]</span> ${renderJsonHtml(value[i], indent + 1)}`;
        if (i < value.length - 1) out += `<span ${tone(COLORS.punctuation, indent + 1)}>,</span>`;
        out += `<br/>`;
      }
      out += `${pad(indent)}<span ${tone(COLORS.bracket, indent)} class="font-semibold">]</span>`;
      return out;
    }
    const keys = Object.keys(value);
    if (keys.length === 0) return `<span ${tone(COLORS.bracket, indent)} class="font-semibold">{ }</span>`;
    let out = `<span ${tone(COLORS.bracket, indent)} class="font-semibold">{</span><br/>`;
    keys.forEach((k, idx) => {
      out += `${pad(indent + 1)}<span ${tone(COLORS.key, indent + 1)}>\"${escapeHtml(k)}\"</span><span ${tone(COLORS.punctuation, indent + 1)}>:</span> ${renderJsonHtml(value[k], indent + 1)}`;
      if (idx < keys.length - 1) out += `<span ${tone(COLORS.punctuation, indent + 1)}>,</span>`;
      out += `<br/>`;
    });
    out += `${pad(indent)}<span ${tone(COLORS.bracket, indent)} class="font-semibold">}</span>`;
    return out;
  };

  const renderPrimitive = (value: any, indent: number) => {
    if (value === null) return <span style={{ color: color(COLORS.null, indent) }}>null</span>;
    if (typeof value === 'boolean') return <span style={{ color: color(COLORS.boolean, indent) }}>{String(value)}</span>;
    if (typeof value === 'number') return <span style={{ color: color(COLORS.number, indent) }}>{String(value)}</span>;
    return <span style={{ color: color(COLORS.string, indent) }}>&quot;{String(value)}&quot;</span>;
  };

  const renderJsonTree = (path: string, value: any, indent: number): React.ReactNode => {
    const linePad = { paddingLeft: `${indent * 16}px` };
    if (value === null || typeof value !== 'object') {
      return (
        <div style={linePad}>{renderPrimitive(value, indent)}</div>
      );
    }
    if (Array.isArray(value)) {
      const collapsed = !!jsonCollapsed[path];
      const header = (
        <div style={linePad} className="cursor-pointer select-none" onClick={() => setJsonCollapsed(prev => ({ ...prev, [path]: !prev[path] }))}>
          {collapsed ? <ChevronRight className="inline h-3 w-3" /> : <ChevronDown className="inline h-3 w-3" />}
          <span style={{ color: color(COLORS.bracket, indent) }} className="font-semibold">[</span>
          <span style={{ color: color(COLORS.index, indent) }}>[{value.length}]</span>
          <span style={{ color: color(COLORS.bracket, indent) }} className="font-semibold">]</span>
        </div>
      );
      if (collapsed) return header;
      return (
        <div>
          {header}
          {value.map((v, i) => (
            <div key={`${path}[${i}]`}>
              <div style={{ paddingLeft: `${(indent + 1) * 16}px` }}>
                <span style={{ color: color(COLORS.index, indent + 1) }}>[{i}]</span>
                <span style={{ color: color(COLORS.punctuation, indent + 1) }}>: </span>
              </div>
              <div>
                {renderJsonTree(`${path}[${i}]`, v, indent + 2)}
              </div>
            </div>
          ))}
        </div>
      );
    }
    const keys = Object.keys(value);
    const collapsed = !!jsonCollapsed[path];
    const header = (
      <div style={linePad} className="cursor-pointer select-none" onClick={() => setJsonCollapsed(prev => ({ ...prev, [path]: !prev[path] }))}>
        {collapsed ? <ChevronRight className="inline h-3 w-3" /> : <ChevronDown className="inline h-3 w-3" />}
        <span style={{ color: color(COLORS.bracket, indent) }} className="font-semibold">{'{'}</span>
        <span style={{ color: color(COLORS.index, indent) }}>{keys.length} keys</span>
        <span style={{ color: color(COLORS.bracket, indent) }} className="font-semibold">{'}'}</span>
      </div>
    );
    if (collapsed) return header;
    return (
      <div>
        {header}
        {keys.map((k, idx) => (
          <div key={`${path}.${k}`}>
            <div style={{ paddingLeft: `${(indent + 1) * 16}px` }}>
              <span style={{ color: color(COLORS.key, indent + 1) }}>&quot;{k}&quot;</span>
              <span style={{ color: color(COLORS.punctuation, indent + 1) }}>: </span>
            </div>
            <div>
              {renderJsonTree(`${path}.${k}`, (value as any)[k], indent + 2)}
            </div>
            {idx < keys.length - 1 && (
              <div style={{ paddingLeft: `${(indent + 1) * 16}px` }}>
                <span style={{ color: color(COLORS.punctuation, indent + 1) }}>,</span>
              </div>
            )}
          </div>
        ))}
      </div>
    );
  };

  const refreshDetail = async () => {
    if (selectedKey) {
      await loadKeyDetail(selectedKey);
    }
  };

  const handleHashUpdate = async (field: string, value: string) => {
    if (!selectedKey) return;
    try {
      await hset(connectionName, selectedKey, field, value, db);
      setHashData(prev => ({ ...prev, [field]: value }));
      toast(t('key_manager.field_updated'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.field_update_fail'), 'error');
    }
  };

  const handleHashDelete = async (field: string) => {
    if (!selectedKey) return;
    try {
      await hdel(connectionName, selectedKey, field, db);
      setHashData(prev => {
        const n = { ...prev };
        delete n[field];
        return n;
      });
      toast(t('key_manager.field_deleted'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.field_delete_fail'), 'error');
    }
  };

  const handleHashAdd = async () => {
    if (!selectedKey || !newHashField) return;
    try {
      await hset(connectionName, selectedKey, newHashField, newHashValue, db);
      setHashData(prev => ({ ...prev, [newHashField]: newHashValue }));
      setNewHashField('');
      setNewHashValue('');
      toast(t('key_manager.field_added'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.field_add_fail'), 'error');
    }
  };

  const handleListPush = async () => {
    if (!selectedKey || !newListValue) return;
    try {
      await lpush(connectionName, selectedKey, newListValue, db);
      setNewListValue('');
      await refreshDetail();
      toast(t('key_manager.item_pushed'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.item_push_fail'), 'error');
    }
  };

  const handleListPop = async () => {
    if (!selectedKey) return;
    try {
      await rpop(connectionName, selectedKey, db);
      await refreshDetail();
      toast(t('key_manager.item_popped'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.item_pop_fail'), 'error');
    }
  };

  const handleSetAdd = async () => {
    if (!selectedKey || !newSetMember) return;
    try {
      await sadd(connectionName, selectedKey, newSetMember, db);
      setNewSetMember('');
      await refreshDetail();
      toast(t('key_manager.member_added'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.member_add_fail'), 'error');
    }
  };

  const handleSetRemove = async (member: string) => {
    if (!selectedKey) return;
    try {
      await srem(connectionName, selectedKey, member, db);
      await refreshDetail();
      toast(t('key_manager.member_removed'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.member_remove_fail'), 'error');
    }
  };

  const handleZsetAdd = async () => {
    if (!selectedKey || !newZsetMember) return;
    const score = parseFloat(newZsetScore || '0');
    try {
      await zadd(connectionName, selectedKey, newZsetMember, score, db);
      setNewZsetMember('');
      setNewZsetScore('');
      await refreshDetail();
      toast(t('key_manager.member_added'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.member_add_fail'), 'error');
    }
  };

  const handleZsetRemove = async (member: string) => {
    if (!selectedKey) return;
    try {
      await zrem(connectionName, selectedKey, member, db);
      await refreshDetail();
      toast(t('key_manager.member_removed'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.member_remove_fail'), 'error');
    }
  };

  const handleJsonSave = async () => {
    if (!selectedKey) return;
    try {
      const parsed = JSON.parse(jsonText || '{}');
      await jsonSet(connectionName, selectedKey, parsed, '$', db);
      toast(t('key_manager.json_saved'), 'success');
    } catch (e: any) {
      toast(e.message || t('key_manager.json_save_fail'), 'error');
    }
  };

  useEffect(() => {
    if (!keyDetail || keyDetail.ttl <= 0) return;

    const timer = setInterval(() => {
      setKeyDetail(prev => {
        if (!prev) return null;
        if (prev.ttl <= 0) return prev;
        return { ...prev, ttl: prev.ttl - 1 };
      });
    }, 1000);

    return () => clearInterval(timer);
  }, [keyDetail?.key, !!(keyDetail && keyDetail.ttl > 0)]);

  const handleUpdateTtl = async () => {
     if (!keyDetail) return;
     const newTtlStr = prompt(t('key_manager.enter_ttl'), keyDetail.ttl.toString());
     if (newTtlStr === null) return;
     
     const newTtl = parseInt(newTtlStr);
     if (isNaN(newTtl)) {
         toast(t('key_manager.invalid_ttl'), "error");
         return;
     }

     try {
         if (newTtl < 0) {
             await persistKey(connectionName, keyDetail.key, db);
         } else {
             await expireKey(connectionName, keyDetail.key, newTtl, db);
         }
         // Reload details to confirm
         loadKeyDetail(keyDetail.key);
         toast(t('key_manager.ttl_updated'), "success");
     } catch (e: any) {
         toast(e.message || t('key_manager.ttl_update_fail'), "error");
     }
  };

  return (
    <div className="flex h-full bg-slate-950 text-slate-100">
      {/* Key List Sidebar */}
      <div className={`${sidebarCollapsed ? 'w-8' : 'w-80'} border-r border-slate-800 flex flex-col transition-all duration-200 ease-in-out`}
      >
        {!sidebarCollapsed ? (
        <div className="p-4 border-b border-slate-800 space-y-3">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 flex-1">
               <Database className="h-4 w-4 text-slate-400" />
               <select 
                 value={db} 
                 onChange={(e) => setDb(parseInt(e.target.value))}
                 className="bg-slate-900 border border-slate-700 text-slate-200 text-sm rounded px-2 py-1 focus:outline-none focus:ring-1 focus:ring-blue-500 w-24"
               >
                 {[...Array(16)].map((_, i) => (
                   <option key={i} value={i}>{t('key_manager.db')} {i}</option>
                 ))}
               </select>
            </div>
            <div className="text-xs text-slate-400 whitespace-nowrap" title="Total keys in current database">
              {dbSize !== null ? `${dbSize} ${t('app.keys')}` : '-'}
            </div>
            <Button size="icon" variant="ghost" onClick={() => setSidebarCollapsed(true)} title="Collapse Sidebar" className="ml-2">
              <ChevronLeft className="h-4 w-4" />
            </Button>
          </div>
          <div className="flex items-center gap-2">
            <button
              className={`text-xs px-2 py-1 rounded ${viewMode === 'type' ? 'bg-blue-600 text-white' : 'bg-slate-900 text-slate-300 border border-slate-700'}`}
              onClick={() => setViewMode('type')}
            >
              {t('key_manager.type')}
            </button>
            <button
              className={`text-xs px-2 py-1 rounded ${viewMode === 'tree' ? 'bg-blue-600 text-white' : 'bg-slate-900 text-slate-300 border border-slate-700'}`}
              onClick={() => {
                setViewMode('tree');
                if (recentPath) {
                  ensureExpandedByPath(recentPath);
                }
              }}
            >
              {t('key_manager.tree')}
            </button>
            <div className="relative inline-flex items-center group">
              <HelpCircle className="h-3 w-3 text-slate-400" aria-label="Help" />
              <div className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 whitespace-nowrap bg-slate-800 text-slate-200 text-xs px-2 py-1 rounded shadow-lg opacity-0 group-hover:opacity-100">
                {t('key_manager.help_type')}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
              <Input 
                 placeholder={t('key_manager.search_placeholder')} 
                 value={pattern}
                 onChange={(e) => setPattern(e.target.value)}
                 className="flex-1"
                 leftIcon={<Search className="h-4 w-4 text-slate-400" />}
              />
              <Button size="icon" variant="ghost" onClick={() => loadKeys(true)}>
                <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
              </Button>
              <div className="flex items-center gap-2">
                <button
                  className={`text-xs px-2 py-1 rounded ${autoRefresh ? 'bg-green-600 text-white' : 'bg-slate-900 text-slate-300 border border-slate-700'}`}
                  onClick={() => setAutoRefresh(v => !v)}
                  title={t('key_manager.auto_refresh')}
                >
                  {t('key_manager.auto_refresh')}
                </button>
                <select
                  className="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded px-2 py-1 focus:outline-none focus:ring-1 focus:ring-blue-500"
                  value={autoIntervalMs}
                  onChange={(e) => setAutoIntervalMs(parseInt(e.target.value))}
                  disabled={!autoRefresh}
                  title="Auto refresh interval"
                >
                  <option value={2000}>2s</option>
                  <option value={5000}>5s</option>
                  <option value={10000}>10s</option>
                  <option value={30000}>30s</option>
                </select>
                <div className="relative inline-flex items-center group">
                  <HelpCircle className="h-3 w-3 text-slate-400" aria-label="Help" />
                  <div className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 whitespace-nowrap bg-slate-800 text-slate-200 text-xs px-2 py-1 rounded shadow-lg opacity-0 group-hover:opacity-100">
                    {t('key_manager.help_auto')}
                  </div>
                </div>
              </div>
            </div>
          <Button className="w-full" onClick={() => setIsAddModalOpen(true)}>
            <Plus className="h-4 w-4 mr-2" /> {t('key_manager.add_key')}
          </Button>
        </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-between py-2">
            <Button size="icon" variant="ghost" onClick={() => setSidebarCollapsed(false)} title="Expand Sidebar">
              <ChevronRightIcon className="h-4 w-4" />
            </Button>
            <div className="text-[10px] text-slate-500 rotate-90 whitespace-nowrap">{t('app.keys')}</div>
            <div />
          </div>
        )}

        {!sidebarCollapsed && (
        <div className="flex-1 overflow-y-auto">
          {keys.length === 0 && !loading && (
            <div className="p-4 text-center text-slate-500 text-sm">
              {t('key_manager.no_keys')}
            </div>
          )}
          {viewMode === 'type' ? (() => {
            const order = ['string','hash','list','set','zset','json','stream','none','unknown'];
            const groups = order.map(t => ({ t, items: keys.filter(k => keyTypes[k] === t) })).filter(g => g.items.length > 0);
            const unknowns = keys.filter(k => !keyTypes[k] || !order.includes(keyTypes[k]));
            if (unknowns.length > 0) groups.push({ t: 'unknown', items: unknowns });
            return (
              <div>
                {groups.map(g => (
                  <div key={g.t}>
                    <div 
                      className="px-3 py-2 bg-slate-900/60 text-xs font-semibold text-slate-400 uppercase tracking-wider sticky top-0 z-10 flex items-center justify-between cursor-pointer"
                      onClick={() => setCollapsedGroups(prev => ({ ...prev, [g.t]: !prev[g.t] }))}
                    >
                      <div className="flex items-center gap-2">
                        {collapsedGroups[g.t] ? (
                          <ChevronRight className="h-3 w-3" />
                        ) : (
                          <ChevronDown className="h-3 w-3" />
                        )}
                        <span>{g.t} ({g.items.length})</span>
                      </div>
                    </div>
                    {!collapsedGroups[g.t] && g.items.map(key => (
                      <div 
                        key={key}
                        className={`p-3 border-b border-slate-800/50 hover:bg-slate-900 cursor-pointer flex items-center justify-between group ${selectedKey === key ? 'bg-slate-900 border-l-2 border-l-blue-500' : ''}`}
                        onClick={() => {
                          const p = key.split(':').join('/');
                          setRecentPath(p);
                          loadKeyDetail(key);
                        }}
                      >
                        <span className="truncate text-sm font-mono flex-1 mr-2" title={key}>{key}</span>
                        <Button 
                          size="icon" 
                          variant="ghost" 
                          className="h-6 w-6 opacity-0 group-hover:opacity-100 text-red-400 hover:text-red-300 hover:bg-red-900/20"
                          onClick={(e) => handleDelete(key, e)}
                        >
                          <Trash2 className="h-3 w-3" />
                        </Button>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            );
          })() : (() => {
            const root: any = { children: {} };
            for (const k of keys) {
              const parts = k.split(':');
              let cur = root;
              for (let i = 0; i < parts.length; i++) {
                const p = parts[i];
                cur.children[p] = cur.children[p] || { children: {}, fullKeys: [] };
                cur = cur.children[p];
                if (i === parts.length - 1) {
                  cur.fullKeys = cur.fullKeys || [];
                  cur.fullKeys.push(k);
                }
              }
            }
            const countKeys = (node: any): number => {
              let c = (node.fullKeys ? node.fullKeys.length : 0);
              for (const n of Object.values(node.children || {})) c += countKeys(n);
              return c;
            };
            const renderBranch = (name: string, node: any, path: string, depth: number) => {
              const collapsed = !!collapsedTree[path];
              return (
                <div key={path}>
                  <div
                    className={`px-3 py-2 bg-slate-900/60 text-xs text-slate-300 sticky top-0 z-10 flex items-center justify-between cursor-pointer`}
                    style={{ paddingLeft: `${12 + depth * 12}px` }}
                    onClick={() => {
                      const next = !collapsedTree[path];
                      setCollapsedTree(prev => ({ ...prev, [path]: next }));
                      if (!next) {
                        setRecentPath(path);
                        ensureExpandedByPath(path);
                      }
                    }}
                  >
                    <div className="flex items-center gap-2">
                      {collapsed ? (
                        <ChevronRight className="h-3 w-3" />
                      ) : (
                        <ChevronDown className="h-3 w-3" />
                      )}
                      <span className="uppercase tracking-wider">{name}</span>
                    </div>
                    <span className="text-[10px] text-slate-500">{countKeys(node)}</span>
                  </div>
                  {!collapsed && (
                    <div>
                      {(node.fullKeys || []).map((key: string) => (
                        <div 
                          key={key}
                          className={`p-3 border-b border-slate-800/50 hover:bg-slate-900 cursor-pointer flex items-center justify-between group ${selectedKey === key ? 'bg-slate-900 border-l-2 border-l-blue-500' : ''}`}
                          onClick={() => {
                            if (viewMode === 'tree') {
                              const p = key.split(':').join('/');
                              setRecentPath(p);
                              ensureExpandedByPath(p);
                            }
                            loadKeyDetail(key);
                          }}
                        >
                          <span className="truncate text-sm font-mono flex-1 mr-2" title={key}>{key}</span>
                          <Button 
                            size="icon" 
                            variant="ghost" 
                            className="h-6 w-6 opacity-0 group-hover:opacity-100 text-red-400 hover:text-red-300 hover:bg-red-900/20"
                            onClick={(e) => handleDelete(key, e)}
                          >
                            <Trash2 className="h-3 w-3" />
                          </Button>
                        </div>
                      ))}
                      {Object.entries(node.children || {}).map(([childName, childNode]: [string, any]) => renderBranch(childName, childNode, `${path}/${childName}`, depth + 1))}
                    </div>
                  )}
                </div>
              );
            };
            return (
              <div>
                {Object.entries(root.children).map(([n, node]: [string, any]) => renderBranch(n, node, n, 0))}
              </div>
            );
          })()}
          {cursor !== 0 && (
             <div className="p-2">
                <Button variant="ghost" className="w-full text-xs" onClick={() => loadKeys(false)} disabled={loading}>
                    {loading ? t('common.loading') : t('key_manager.load_more')}
                </Button>
             </div>
          )}
        </div>
        )}
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col bg-slate-950 min-w-0">
        {selectedKey && keyDetail ? (
          <>
            {/* Header */}
            <div className="p-4 border-b border-slate-800 flex items-center justify-between bg-slate-900/50">
               <div className="flex items-center gap-4 overflow-hidden">
                 <h2 className="text-lg font-mono font-semibold truncate text-blue-400">{keyDetail.key}</h2>
                 <div className="flex items-center text-xs text-slate-400 bg-slate-800 px-2 py-1 rounded cursor-pointer hover:text-slate-200" onClick={handleUpdateTtl} title="Click to edit TTL">
                    <Clock className="h-3 w-3 mr-1" />
                    {keyDetail.ttl === -1 ? t('key_manager.persist') : `${keyDetail.ttl}s`}
                 </div>
               </div>
               <div className="flex items-center gap-2">
                 <Button variant="primary" size="sm" onClick={handleUpdateValue}>
                    <Save className="h-4 w-4 mr-1" /> {t('common.save')}
                 </Button>
                 <Button variant="danger" size="sm" onClick={() => handleDelete(selectedKey)}>
                    <Trash2 className="h-4 w-4 mr-1" /> {t('common.delete')}
                 </Button>
               </div>
            </div>

            <div className="flex-1 p-4 overflow-hidden flex flex-col relative min-h-0 min-w-0">
              {detailLoading && (
                <div className="absolute inset-0 bg-slate-950/50 flex items-center justify-center z-10">
                  <RefreshCw className="h-6 w-6 animate-spin text-blue-500" />
                </div>
              )}
              {keyType === 'string' && (
                <>
                  <label className="text-xs font-semibold text-slate-500 mb-2 uppercase tracking-wider">{t('key_manager.string_value')}</label>
                  <div className="flex items-center gap-2 mb-2">
                    {canFormatJson && (
                      <Button size="sm" onClick={toggleStringFormat}>
                        <Wand2 className="h-4 w-4 mr-1" /> {stringFormatEnabled ? t('key_manager.cancel_format') : t('key_manager.format_json')}
                      </Button>
                    )}
                    {stringFormatEnabled && (
                      <Button size="sm" variant="ghost" onClick={refreshStringFormat}>
                        <RefreshCw className="h-4 w-4 mr-1" /> {t('key_manager.refresh_preview')}
                      </Button>
                    )}
                    <Button size="sm" variant="ghost" onClick={() => copyText(stringFormatEnabled ? stringFormattedText : (keyDetail.value || ''))}>
                      <Copy className="h-4 w-4 mr-1" /> {stringFormatEnabled ? t('key_manager.copy_formatted') : t('key_manager.copy_original')}
                    </Button>
                  </div>
                  {!stringFormatEnabled && (
                    <textarea
                      className="flex-1 w-full bg-slate-900 border border-slate-700 rounded-md p-4 font-mono text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-blue-500/50 resize-none"
                      wrap="soft"
                      value={keyDetail.value || ''}
                      onChange={(e) => setKeyDetail({ ...keyDetail, value: e.target.value })}
                    />
                  )}
                  {stringFormatEnabled && (
                    <div className="mt-2 flex-1 min-h-0 w-full bg-slate-900 border border-slate-700 rounded-md p-4 font-mono text-sm text-slate-200 overflow-x-auto overflow-y-auto whitespace-pre">
                      {stringFormattedObject && (
                        <div>
                          {renderJsonTree('root', stringFormattedObject, 0)}
                        </div>
                      )}
                    </div>
                  )}
                </>
              )}
              {keyType === 'hash' && (
                <div className="flex-1 overflow-y-auto">
                  <div className="space-y-2">
                    {Object.entries(hashData).map(([f, v]) => (
                      <div key={f} className="flex items-center gap-2">
                        <span className="w-48 truncate text-xs font-mono bg-slate-800 px-2 py-1 rounded" title={f}>{f}</span>
                        <input className="flex-1 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" value={v} onChange={(e) => setHashData(prev => ({ ...prev, [f]: e.target.value }))} />
                        <Button size="sm" onClick={() => handleHashUpdate(f, hashData[f])}>{t('common.save')}</Button>
                        <Button size="sm" variant="danger" onClick={() => handleHashDelete(f)}>{t('common.delete')}</Button>
                      </div>
                    ))}
                    <div className="flex items-center gap-2 mt-4">
                      <input className="w-48 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" placeholder={t('key_manager.placeholder_field')} value={newHashField} onChange={(e) => setNewHashField(e.target.value)} />
                      <input className="flex-1 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" placeholder={t('key_manager.placeholder_value')} value={newHashValue} onChange={(e) => setNewHashValue(e.target.value)} />
                      <Button size="sm" onClick={handleHashAdd}>{t('common.add')}</Button>
                    </div>
                  </div>
                </div>
              )}
              {keyType === 'list' && (
                <div className="flex-1 overflow-y-auto">
                  <div className="space-y-2">
                    {listData.map((item, idx) => (
                      <div key={idx} className="flex items-center gap-2">
                        <span className="flex-1 truncate text-sm font-mono bg-slate-800 px-2 py-1 rounded" title={item}>{item}</span>
                      </div>
                    ))}
                    <div className="flex items-center gap-2 mt-4">
                      <input className="flex-1 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" placeholder={t('key_manager.placeholder_value')} value={newListValue} onChange={(e) => setNewListValue(e.target.value)} />
                      <Button size="sm" onClick={handleListPush}>LPUSH</Button>
                      <Button size="sm" variant="danger" onClick={handleListPop}>RPOP</Button>
                    </div>
                  </div>
                </div>
              )}
              {keyType === 'set' && (
                <div className="flex-1 overflow-y-auto">
                  <div className="space-y-2">
                    {setData.map((m) => (
                      <div key={m} className="flex items-center gap-2">
                        <span className="flex-1 truncate text-sm font-mono bg-slate-800 px-2 py-1 rounded" title={m}>{m}</span>
                        <Button size="sm" variant="danger" onClick={() => handleSetRemove(m)}>{t('common.remove')}</Button>
                      </div>
                    ))}
                    <div className="flex items-center gap-2 mt-4">
                      <input className="flex-1 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" placeholder={t('key_manager.placeholder_member')} value={newSetMember} onChange={(e) => setNewSetMember(e.target.value)} />
                      <Button size="sm" onClick={handleSetAdd}>SADD</Button>
                    </div>
                  </div>
                </div>
              )}
              {keyType === 'zset' && (
                <div className="flex-1 overflow-y-auto">
                  <div className="space-y-2">
                    {zsetData.map(([m, s]) => (
                      <div key={m} className="flex items-center gap-2">
                        <span className="flex-1 truncate text-sm font-mono bg-slate-800 px-2 py-1 rounded" title={m}>{m}</span>
                        <span className="text-xs bg-slate-700 px-2 py-1 rounded">{s}</span>
                        <Button size="sm" variant="danger" onClick={() => handleZsetRemove(m)}>{t('common.remove')}</Button>
                      </div>
                    ))}
                    <div className="flex items-center gap-2 mt-4">
                      <input className="flex-1 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" placeholder={t('key_manager.placeholder_member')} value={newZsetMember} onChange={(e) => setNewZsetMember(e.target.value)} />
                      <input className="w-32 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm" placeholder={t('key_manager.placeholder_score')} value={newZsetScore} onChange={(e) => setNewZsetScore(e.target.value)} />
                      <Button size="sm" onClick={handleZsetAdd}>ZADD</Button>
                    </div>
                  </div>
                </div>
              )}
              {keyType === 'json' && (
                <div className="flex-1 overflow-hidden flex flex-col">
                  <label className="text-xs font-semibold text-slate-500 mb-2 uppercase tracking-wider">{t('key_manager.json_value')}</label>
                  <textarea className="flex-1 w-full bg-slate-900 border border-slate-700 rounded-md p-4 font-mono text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-blue-500/50 resize-none" value={jsonText} onChange={(e) => setJsonText(e.target.value)} />
                  <div className="mt-2">
                    <Button size="sm" onClick={handleJsonSave}>{t('key_manager.save_json')}</Button>
                  </div>
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-slate-500 flex-col gap-4">
             <div className="p-6 rounded-full bg-slate-900">
                <Search className="h-12 w-12 opacity-20" />
             </div>
             <p>{t('key_manager.select_key')}</p>
          </div>
        )}
      </div>

      {/* Add Key Modal */}
      <Modal isOpen={isAddModalOpen} onClose={() => setIsAddModalOpen(false)} title={t('key_manager.add_new_key')}>
         <div className="space-y-4">
            <div className="space-y-2">
               <label className="text-sm font-medium text-slate-300">{t('key_manager.key_name')}</label>
               <Input 
                  placeholder={t('key_manager.placeholder_key_example')} 
                  value={newKeyName}
                  onChange={(e) => setNewKeyName(e.target.value)}
               />
            </div>
            <div className="space-y-2">
               <label className="text-sm font-medium text-slate-300">{t('key_manager.type')}</label>
               <select 
                  className="w-full bg-slate-950 border border-slate-700 rounded-md p-2 text-sm"
                  value={newKeyType}
                  onChange={(e) => setNewKeyType(e.target.value as any)}
               >
                  <option value="string">string</option>
                  <option value="hash">hash</option>
                  <option value="list">list</option>
                  <option value="set">set</option>
                  <option value="zset">zset</option>
                  <option value="json">json</option>
               </select>
               <div className="flex items-center gap-2">
                 <div className="relative inline-flex items-center group">
                   <HelpCircle className="h-3 w-3 text-slate-400" aria-label="Help" />
                   <div className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 whitespace-nowrap bg-slate-800 text-slate-200 text-xs px-2 py-1 rounded shadow-lg opacity-0 group-hover:opacity-100">
                     {t('key_manager.help_key_type')}
                   </div>
                 </div>
               </div>
            </div>
            {newKeyType === 'string' && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('key_manager.value')}</label>
                <textarea 
                  className="w-full h-32 bg-slate-950 border border-slate-700 rounded-md p-3 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder={t('key_manager.enter_value_placeholder')}
                  value={newKeyValue}
                  onChange={(e) => setNewKeyValue(e.target.value)}
                />
              </div>
            )}
            {newKeyType === 'hash' && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('key_manager.field_and_value')}</label>
                <div className="flex items-center gap-2">
                  <Input className="w-48" placeholder={t('key_manager.placeholder_field')} value={addHashField} onChange={(e) => setAddHashField(e.target.value)} />
                  <Input className="flex-1" placeholder={t('key_manager.placeholder_value')} value={addHashValue} onChange={(e) => setAddHashValue(e.target.value)} />
                </div>
              </div>
            )}
            {newKeyType === 'list' && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('key_manager.initial_value')}</label>
                <Input placeholder={t('key_manager.placeholder_value')} value={addListValue} onChange={(e) => setAddListValue(e.target.value)} />
              </div>
            )}
            {newKeyType === 'set' && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('key_manager.member')}</label>
                <Input placeholder={t('key_manager.placeholder_member')} value={addSetMember} onChange={(e) => setAddSetMember(e.target.value)} />
              </div>
            )}
            {newKeyType === 'zset' && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">{t('key_manager.member_and_score')}</label>
                <div className="flex items-center gap-2">
                  <Input className="flex-1" placeholder={t('key_manager.placeholder_member')} value={addZsetMember} onChange={(e) => setAddZsetMember(e.target.value)} />
                  <Input className="w-32" placeholder={t('key_manager.placeholder_score')} value={addZsetScore} onChange={(e) => setAddZsetScore(e.target.value)} />
                </div>
              </div>
            )}
            {newKeyType === 'json' && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-300">JSON</label>
                <textarea 
                  className="w-full h-32 bg-slate-950 border border-slate-700 rounded-md p-3 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder={t('key_manager.placeholder_json_example')}
                  value={addJsonText}
                  onChange={(e) => setAddJsonText(e.target.value)}
                />
              </div>
            )}
            <div className="space-y-2">
               <label className="text-sm font-medium text-slate-300">{t('key_manager.ttl_label_optional')}</label>
               <Input 
                  type="number"
                  placeholder={t('key_manager.ttl_placeholder')}
                  value={newKeyTtl}
                  onChange={(e) => setNewKeyTtl(e.target.value)}
               />
               <div className="flex items-center gap-2">
                 <div className="relative inline-flex items-center group">
                   <HelpCircle className="h-3 w-3 text-slate-400" aria-label="Help" />
                   <div className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 whitespace-nowrap bg-slate-800 text-slate-200 text-xs px-2 py-1 rounded shadow-lg opacity-0 group-hover:opacity-100">
                     {t('key_manager.help_ttl')}
                   </div>
                 </div>
               </div>
            </div>
            <div className="flex justify-end gap-2 mt-6">
               <Button variant="ghost" onClick={() => setIsAddModalOpen(false)}>{t('common.cancel')}</Button>
               <Button onClick={handleAddKey}>{t('key_manager.create_key')}</Button>
            </div>
         </div>
      </Modal>
    </div>
  );
}
