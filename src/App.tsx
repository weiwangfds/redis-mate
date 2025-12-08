import { useState, useEffect as useEffectReact } from "react";
import { useTranslation } from "react-i18next";
import { ConnectionManager } from "./components/ConnectionManager";
import { KeyManager } from "./components/KeyManager";
import { PubSubPanel } from "./components/PubSubPanel";
import { ClusterInfoPanel } from "./components/ClusterInfoPanel";
import { ToastProvider, useToast } from "./components/ui/Toast";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
import { Database, Radio, Network, ChevronUp, ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "./utils";
import { getConfig } from "./types/tauri";
import { useEffect } from "react";

type View = 'keys' | 'pubsub' | 'cluster';

export default function App() {
  const { t } = useTranslation();
  const { toast } = useToast();
  const [selectedConnection, setSelectedConnection] = useState<string>("");
  const [currentView, setCurrentView] = useState<View>('keys');
  const [isCluster, setIsCluster] = useState(false);
  const [headerCollapsed, setHeaderCollapsed] = useState(false);
  const [connCollapsed, setConnCollapsed] = useState(false);

  useEffect(() => {
    if (selectedConnection) {
      getConfig(selectedConnection).then(cfg => {
        setIsCluster(cfg?.cluster || false);
      });
    } else {
      setIsCluster(false);
    }
  }, [selectedConnection]);

  useEffectReact(() => {
    try {
      const raw = localStorage.getItem('app:prefs');
      if (raw) {
        const s = JSON.parse(raw);
        if (typeof s.headerCollapsed === 'boolean') setHeaderCollapsed(s.headerCollapsed);
        if (typeof s.connCollapsed === 'boolean') setConnCollapsed(s.connCollapsed);
      }
    } catch {}
  }, []);

  useEffectReact(() => {
    try {
      const s = { headerCollapsed, connCollapsed };
      localStorage.setItem('app:prefs', JSON.stringify(s));
    } catch {}
  }, [headerCollapsed, connCollapsed]);

  useEffectReact(() => {
    (async () => {
      try {
        const update = await checkUpdate();
        if (update?.available) {
          toast('Update available. Downloading...', 'info');
          await update.downloadAndInstall();
          toast('Update installed. Restart the app to apply.', 'success');
        }
      } catch {}
    })();
  }, []);

  return (
    <ToastProvider>
      <div className="flex h-screen w-screen bg-slate-950 text-slate-100 overflow-hidden font-sans">
        {connCollapsed ? (
          <div className="w-6 bg-slate-900 border-r border-slate-800 flex flex-col items-center justify-between py-2">
            <button className="text-slate-400 hover:text-slate-200" title="Expand Connections" onClick={() => setConnCollapsed(false)}>
              <ChevronRight className="h-4 w-4" />
            </button>
            <div className="text-[10px] text-slate-500 rotate-90 whitespace-nowrap">{t('app.connections')}</div>
            <div />
          </div>
        ) : (
          <ConnectionManager 
            selectedName={selectedConnection} 
            onSelect={setSelectedConnection} 
            onCollapse={() => setConnCollapsed(true)}
          />
        )}
        
        <div className="flex-1 flex flex-col min-w-0">
          {selectedConnection ? (
            <>
              {/* Header / Tabs */}
              {headerCollapsed ? (
                <div className="h-6 border-b border-slate-800 flex items-center px-2 bg-slate-900/50">
                  <button className="text-slate-400 hover:text-slate-200" title="Expand Header" onClick={() => setHeaderCollapsed(false)}>
                    <ChevronDown className="h-4 w-4" />
                  </button>
                  <div className="ml-2 text-xs text-slate-500 truncate">
                    {t('app.connected_to')} <span className="text-blue-400 font-semibold">{selectedConnection}</span>
                  </div>
                </div>
              ) : (
                <div className="h-12 border-b border-slate-800 flex items-center px-4 bg-slate-900/50">
                  <div className="flex space-x-1 bg-slate-800 p-1 rounded-lg">
                    <button
                      onClick={() => setCurrentView('keys')}
                      className={cn(
                        "px-3 py-1 text-sm font-medium rounded-md flex items-center gap-2 transition-all",
                        currentView === 'keys' 
                          ? "bg-slate-700 text-white shadow-sm" 
                          : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
                      )}
                    >
                      <Database className="h-4 w-4" /> {t('app.keys')}
                    </button>
                    <button
                      onClick={() => setCurrentView('pubsub')}
                      className={cn(
                        "px-3 py-1 text-sm font-medium rounded-md flex items-center gap-2 transition-all",
                        currentView === 'pubsub' 
                          ? "bg-slate-700 text-white shadow-sm" 
                          : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
                      )}
                    >
                      <Radio className="h-4 w-4" /> {t('app.pubsub')}
                    </button>
                    {isCluster && (
                      <button
                        onClick={() => setCurrentView('cluster')}
                        className={cn(
                          "px-3 py-1 text-sm font-medium rounded-md flex items-center gap-2 transition-all",
                          currentView === 'cluster' 
                            ? "bg-slate-700 text-white shadow-sm" 
                            : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
                        )}
                      >
                        <Network className="h-4 w-4" /> {t('app.cluster')}
                      </button>
                    )}
                  </div>
                  <div className="ml-auto text-sm text-slate-500 flex items-center gap-3">
                    <div>
                      {t('app.connected_to')} <span className="text-blue-400 font-semibold">{selectedConnection}</span>
                    </div>
                    <button className="text-slate-400 hover:text-slate-200" title="Collapse Header" onClick={() => setHeaderCollapsed(true)}>
                      <ChevronUp className="h-4 w-4" />
                    </button>
                  </div>
                </div>
              )}

              {/* Content */}
              <div className="flex-1 overflow-hidden">
                {currentView === 'keys' && <KeyManager connectionName={selectedConnection} />}
                {currentView === 'pubsub' && <PubSubPanel connectionName={selectedConnection} />}
                {currentView === 'cluster' && isCluster && <ClusterInfoPanel connectionName={selectedConnection} />}
              </div>
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center text-slate-500 flex-col gap-4">
               <div className="p-8 rounded-full bg-slate-900">
                  <svg className="w-16 h-16 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                  </svg>
               </div>
               <p className="text-lg font-medium">{t('app.select_connection')}</p>
            </div>
          )}
        </div>
      </div>
    </ToastProvider>
  );
}
