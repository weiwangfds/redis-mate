import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getClusterInfo, ClusterNodeInfo } from '../types/tauri';
import { Card, CardHeader, CardTitle, CardContent } from './ui/Card';
import { Button } from './ui/Button';
import { RefreshCw, Server, Database } from 'lucide-react';
import { cn } from '../utils';
import { useToast } from './ui/Toast';

interface ClusterInfoPanelProps {
  connectionName: string;
}

export function ClusterInfoPanel({ connectionName }: ClusterInfoPanelProps) {
  const { t } = useTranslation();
  const [nodes, setNodes] = useState<ClusterNodeInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const { toast } = useToast();

  const fetchData = async () => {
    setLoading(true);
    try {
      const info = await getClusterInfo(connectionName);
      // Sort: masters first, then by ID
      info.sort((a, b) => {
        const aMaster = a.flags.includes('master');
        const bMaster = b.flags.includes('master');
        if (aMaster && !bMaster) return -1;
        if (!aMaster && bMaster) return 1;
        return a.id.localeCompare(b.id);
      });
      setNodes(info);
    } catch (e: any) {
      toast(e.message || t('cluster.fetch_fail') || 'Failed to fetch cluster info', 'error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, [connectionName]);

  const masters = nodes.filter(n => n.flags.includes('master'));
  const slaves = nodes.filter(n => n.flags.includes('slave'));

  return (
    <div className="h-full flex flex-col p-4 space-y-4 overflow-y-auto bg-slate-950">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold text-slate-100 flex items-center gap-2">
          <Database className="h-5 w-5 text-blue-500" />
          {t('cluster.topology')}
        </h2>
        <Button 
          variant="outline" 
          size="sm" 
          onClick={fetchData} 
          disabled={loading}
          className="gap-2"
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
          {t('common.refresh')}
        </Button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {masters.map(master => (
          <Card key={master.id} className="bg-slate-900 border-slate-800">
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium text-blue-400 flex items-center gap-2">
                <Server className="h-4 w-4" />
                {t('cluster.master')}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm">
              <div className="flex justify-between text-slate-400">
                <span>{t('cluster.id')}:</span>
                <span className="text-slate-200 font-mono text-xs">{master.id.substring(0, 8)}...</span>
              </div>
              <div className="flex justify-between text-slate-400">
                <span>{t('cluster.address')}:</span>
                <span className="text-slate-200">{master.addr}</span>
              </div>
              <div className="mt-2 pt-2 border-t border-slate-800">
                <span className="text-slate-400 block mb-1">{t('cluster.slots')} ({master.slots.length}):</span>
                <div className="flex flex-wrap gap-1">
                  {master.slots.map((slot, i) => (
                    <span key={i} className="inline-block px-1.5 py-0.5 bg-blue-500/10 text-blue-400 rounded text-xs font-mono">
                      {slot}
                    </span>
                  ))}
                  {master.slots.length === 0 && <span className="text-slate-600 italic">{t('cluster.no_slots')}</span>}
                </div>
              </div>

              {/* Slaves */}
              <div className="mt-2 pt-2 border-t border-slate-800">
                <span className="text-slate-400 block mb-1">{t('cluster.replicas')}:</span>
                <div className="space-y-1">
                  {slaves.filter(s => s.master_id === master.id).map(slave => (
                    <div key={slave.id} className="flex items-center gap-2 text-xs bg-slate-800/50 p-1.5 rounded">
                       <div className="w-2 h-2 rounded-full bg-green-500/50" />
                       <span className="font-mono text-slate-300">{slave.addr}</span>
                    </div>
                  ))}
                  {slaves.filter(s => s.master_id === master.id).length === 0 && (
                    <span className="text-slate-600 italic text-xs">{t('cluster.no_replicas')}</span>
                  )}
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
