import { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { cn } from '../../utils';
import { X, CheckCircle, AlertCircle, Info } from 'lucide-react';

type ToastType = 'success' | 'error' | 'info';

interface Toast {
  id: string;
  message: string;
  type: ToastType;
}

interface ToastContextValue {
  toast: (message: string, type?: ToastType) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) throw new Error('useToast must be used within a ToastProvider');
  return context;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const toast = useCallback((message: string, type: ToastType = 'info') => {
    const id = Math.random().toString(36).substring(2);
    setToasts((prev) => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3000);
  }, []);

  return (
    <ToastContext.Provider value={{ toast }}>
      {children}
      <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={cn(
              "flex items-center gap-2 rounded-md border p-4 text-sm font-medium shadow-lg animate-in slide-in-from-right-full",
              {
                'border-green-500/20 bg-green-500/10 text-green-500': t.type === 'success',
                'border-red-500/20 bg-red-500/10 text-red-500': t.type === 'error',
                'border-blue-500/20 bg-blue-500/10 text-blue-500': t.type === 'info',
              }
            )}
          >
            {t.type === 'success' && <CheckCircle className="h-4 w-4" />}
            {t.type === 'error' && <AlertCircle className="h-4 w-4" />}
            {t.type === 'info' && <Info className="h-4 w-4" />}
            {t.message}
            <button
              onClick={() => setToasts((prev) => prev.filter((item) => item.id !== t.id))}
              className="ml-auto hover:opacity-70"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
