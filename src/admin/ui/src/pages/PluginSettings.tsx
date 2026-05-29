import { useState, useMemo, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX, getQueryClient } from '../lib/api';
import { useI18n } from '../i18n';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Select } from '../components/Select';

interface SettingDef {
  key: string;
  label: string;
  setting_type: string;
  default?: string;
  description?: string;
  options?: { value: string; label: string }[];
}

interface PluginSettingsResponse {
  plugin_name: string;
  settings: SettingDef[];
  values: Record<string, string>;
}

/* ── 样式常量（与 Settings.tsx 风格一致） ── */
const sectionStyle: React.CSSProperties = {
  background: 'var(--md-surface-container-lowest)',
  borderRadius: 'var(--radius-lg)',
  marginBottom: '20px',
};

const secHeadStyle: React.CSSProperties = {
  padding: '18px 24px',
  background: 'var(--md-surface-container-low)',
};

const secTitleStyle: React.CSSProperties = {
  fontSize: '15px',
  fontWeight: 700,
  color: 'var(--md-on-surface)',
  letterSpacing: '-0.2px',
};

const secBodyStyle: React.CSSProperties = {
  padding: '24px',
  display: 'flex',
  flexDirection: 'column' as const,
  gap: '18px',
};

const formRowStyle: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: '180px 1fr',
  gap: '12px',
  alignItems: 'start',
};

const labelStyle: React.CSSProperties = {
  fontSize: '13.5px',
  fontWeight: 600,
  color: 'var(--md-on-surface-variant)',
  paddingTop: '10px',
};

const hintStyle: React.CSSProperties = {
  fontSize: '12px',
  color: 'var(--md-outline)',
  opacity: 0.8,
  marginTop: '4px',
};

const checkboxRowStyle: React.CSSProperties = {
  paddingTop: '10px',
};

const CENTER_STYLE: React.CSSProperties = {
  minHeight: '60vh',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  color: 'var(--md-on-surface-variant)',
  fontSize: '14px',
};

function FormRow({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div style={formRowStyle}>
      <span style={labelStyle}>{label}</span>
      <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '5px' }}>
        {children}
        {hint && <span style={hintStyle}>{hint}</span>}
      </div>
    </div>
  );
}

function renderControl(
  s: SettingDef,
  value: string,
  onChange: (val: string) => void,
) {
  switch (s.setting_type) {
    case 'text':
      return <Input value={value || ''} onChange={(e) => onChange(e.target.value)} />;
    case 'textarea':
      return (
        <textarea
          rows={4}
          value={value || ''}
          onChange={(e) => onChange(e.target.value)}
          style={{
            width: '100%',
            padding: '10px 14px',
            border: 'none',
            borderRadius: 'var(--radius-md)',
            fontSize: '14px',
            color: 'var(--md-on-surface)',
            background: 'var(--md-surface-container-low)',
            outline: 'none',
            resize: 'vertical' as const,
            fontFamily: 'inherit',
            boxSizing: 'border-box' as const,
            transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
          }}
          onFocus={(e) => {
            e.currentTarget.style.boxShadow = '0 0 0 2px rgba(249,115,22,0.2)';
            e.currentTarget.style.background = 'var(--md-surface-container-lowest)';
          }}
          onBlur={(e) => {
            e.currentTarget.style.boxShadow = 'none';
            e.currentTarget.style.background = 'var(--md-surface-container-low)';
          }}
        />
      );
    case 'bool':
      return (
        <div style={checkboxRowStyle}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={value === 'true'}
              onChange={(e) => onChange(e.target.checked ? 'true' : 'false')}
              style={{ width: '18px', height: '18px', accentColor: 'var(--md-primary)' }}
            />
            <span style={{ fontSize: '13px', color: 'var(--md-on-surface-variant)' }}>启用</span>
          </label>
        </div>
      );
    case 'number':
      return <Input type="number" value={value || ''} onChange={(e) => onChange(e.target.value)} />;
    case 'select':
      if (!s.options || s.options.length === 0) {
        return <Input value={value || ''} onChange={(e) => onChange(e.target.value)} />;
      }
      return (
        <Select value={value || s.options[0].value} onChange={(e) => onChange(e.target.value)}>
          {s.options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </Select>
      );
    default:
      return <Input value={value || ''} onChange={(e) => onChange(e.target.value)} />;
  }
}

export default function PluginSettings() {
  const { name } = useParams<{ name: string }>();
  const navigate = useNavigate();
  const { t } = useI18n();

  const [dirtyValues, setDirtyValues] = useState<Record<string, string>>({});
  const [error, setError] = useState('');
  const lastDefaultsRef = useRef<string>('');

  const { data: settingsData, isLoading } = useQuery({
    queryKey: ['pluginSettings', name],
    queryFn: () => apiData<PluginSettingsResponse>(`${API_PREFIX}/admin/plugins/${name}/settings`),
    enabled: !!name,
    staleTime: 0,
  });

  // 从 API 响应计算默认值
  const defaultValues = useMemo(() => {
    if (!settingsData) return {};
    const merged: Record<string, string> = { ...settingsData.values };
    for (const s of settingsData.settings) {
      if (!(s.key in merged) && s.default !== undefined) {
        merged[s.key] = s.default;
      }
    }
    return merged;
  }, [settingsData]);

  // 首次加载时重置 dirty values
  const defaultsKey = JSON.stringify(Object.keys(defaultValues).sort());
  if (lastDefaultsRef.current !== defaultsKey) {
    lastDefaultsRef.current = defaultsKey;
    // 延迟到下一个微任务重置 state，避免在 render 中 setState
    queueMicrotask(() => setDirtyValues({}));
  }

  // 当前值 = 默认值覆盖用户修改
  const currentValues: Record<string, string> = { ...defaultValues, ...dirtyValues };

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!name) return;
      await apiData<{ updated: boolean }>(`${API_PREFIX}/admin/plugins/${name}/settings`, {
        method: 'PUT',
        body: JSON.stringify({ settings: currentValues }),
      });
    },
    onSuccess: () => {
      getQueryClient().invalidateQueries({ queryKey: ['pluginSettings', name] });
      navigate('/admin/plugins');
    },
    onError: (e) => {
      setError(e instanceof Error ? e.message : String(e));
    },
  });

  const update = (key: string, value: string) => {
    setDirtyValues((prev) => ({ ...prev, [key]: value }));
  };

  const handleSave = () => {
    setError('');
    saveMutation.mutate();
  };

  const schema = settingsData?.settings ?? [];

  /* ── Loading ── */
  if (isLoading) {
    return <div style={CENTER_STYLE}>Loading...</div>;
  }

  /* ── Error ── */
  if (error && schema.length === 0) {
    return (
      <div style={{ ...CENTER_STYLE, flexDirection: 'column' as const, gap: '16px' }}>
        <div style={{ color: 'var(--danger-500)', fontSize: '15px', fontWeight: 600 }}>错误</div>
        <div style={{ color: 'var(--md-on-surface-variant)' }}>{error}</div>
        <Button variant="ghost" onClick={() => navigate('/admin/plugins')}>
          返回插件列表
        </Button>
      </div>
    );
  }

  /* ── Empty schema ── */
  if (schema.length === 0) {
    return (
      <>
        <PageHeader title={name || '插件'} subtitle="此插件没有可配置的选项" />
        <div style={CENTER_STYLE}>
          <div style={{ color: 'var(--md-outline)' }}>该插件未定义任何设置项</div>
        </div>
      </>
    );
  }

  return (
    <>
      <PageHeader
        title={name || t('pluginSettings')}
        subtitle="配置插件运行参数"
        actions={
          <Button onClick={handleSave} disabled={saveMutation.isPending} loading={saveMutation.isPending}>
            保存
          </Button>
        }
      />

      {error && (
        <div
          style={{
            padding: '12px 16px',
            marginBottom: '20px',
            borderRadius: 'var(--radius-md)',
            background: 'rgba(239,68,68,0.08)',
            color: 'var(--danger-600)',
            fontSize: '13px',
          }}
        >
          {error}
        </div>
      )}

      <div style={sectionStyle}>
        <div style={secHeadStyle}>
          <h3 style={secTitleStyle}>插件设置</h3>
        </div>
        <div style={secBodyStyle}>
          {schema.map((s) => (
            <FormRow key={s.key} label={s.label} hint={s.description}>
              {renderControl(s, currentValues[s.key] ?? '', (val) => update(s.key, val))}
            </FormRow>
          ))}
        </div>
      </div>
    </>
  );
}
