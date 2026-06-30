import { useState, useEffect } from 'react';
import { Card, Button, Typography, Space, Tag, message, Row, Col, theme } from 'antd';
import {
  CrownOutlined, StarOutlined, SafetyOutlined, ThunderboltOutlined,
  CheckCircleFilled, CloseCircleOutlined,
} from '@ant-design/icons';
import { useAuth } from '../../hooks/useApi';
import { writeAuthFile } from '../../api';
import { useI18n } from '../../i18n';

const { Text, Title } = Typography;

// ── Auth plan definitions ──

interface PlanFeature {
  label: string;
  included: boolean;
}

interface Plan {
  id: string;
  name: string;
  icon: React.ReactNode;
  color: string;
  badgeColor: string;
  description: string;
  json: Record<string, unknown>;
  features: PlanFeature[];
}

const planDefs = (t: (k: string, p?: Record<string, string | number>) => string): Plan[] => [
  {
    id: 'free',
    name: 'Free',
    icon: <SafetyOutlined />,
    color: '#6b7280',
    badgeColor: '#4b5563',
    description: t('plan_free'),
    json: {
      access_token: 'fake-free-token',
      expires_at: 2099000000,
      subscription: {
        plan: 'free',
        tier: 'basic',
        usage_multiplier: 1,
        features: ['sandbox-safe', 'codegraph-lite'],
      },
    },
    features: [
      { label: t('feature_safe_sandbox'), included: true },
      { label: t('feature_codegraph_lite'), included: true },
      { label: t('feature_browser'), included: false },
      { label: t('feature_mcp'), included: false },
      { label: t('feature_computer_use'), included: false },
      { label: t('feature_memory'), included: false },
      { label: t('feature_1m_context'), included: false },
    ],
  },
  {
    id: 'plus',
    name: 'Plus',
    icon: <StarOutlined />,
    color: "#22c55e",
    badgeColor: '#16a34a',
    description: t('plan_plus'),
    json: {
      access_token: 'fake-plus-token',
      expires_at: 2099000000,
      subscription: {
        plan: 'plus',
        tier: 'individual',
        usage_multiplier: 1,
        features: [
          'sandbox-basic',
          'browser-plugin-basic',
          'mcp-basic',
          'codegraph-lite',
          '128k-context',
        ],
      },
    },
    features: [
      { label: t('feature_basic_sandbox'), included: true },
      { label: t('feature_basic_browser'), included: true },
      { label: t('feature_basic_mcp'), included: true },
      { label: t('feature_codegraph_lite'), included: true },
      { label: t('feature_128k_context'), included: true },
      { label: t('feature_computer_use'), included: false },
      { label: t('feature_1m_context'), included: false },
    ],
  },
  {
    id: 'pro',
    name: 'Pro',
    icon: <CrownOutlined />,
    color: '#f59e0b',
    badgeColor: '#d97706',
    description: t('plan_pro'),
    json: {
      access_token: 'fake-pro-token',
      expires_at: 2099000000,
      subscription: {
        plan: 'pro',
        tier: 'power-user',
        usage_multiplier: 5,
        features: [
          'sandbox-danger-full-access',
          'browser-full-automation',
          'computer-use',
          'mcp-server-full',
          'codegraph-full',
          'agent-memory',
          'long-context-1M',
        ],
      },
    },
    features: [
      { label: t('feature_full_sandbox'), included: true },
      { label: t('feature_full_browser'), included: true },
      { label: t('feature_computer_use'), included: true },
      { label: t('feature_full_mcp'), included: true },
      { label: t('feature_full_codegraph'), included: true },
      { label: t('feature_full_memory'), included: true },
      { label: t('feature_1m'), included: true },
    ],
  },
  {
    id: 'enterprise',
    name: 'Enterprise',
    icon: <ThunderboltOutlined />,
    color: '#ef4444',
    badgeColor: '#dc2626',
    description: t('plan_enterprise'),
    json: {
      access_token: 'fake-enterprise-token',
      expires_at: 2099000000,
      subscription: {
        plan: 'enterprise',
        tier: 'org-admin',
        usage_multiplier: 999,
        unlimited_quota: true,
        features: [
          'sandbox-danger-full-access',
          'browser-full-automation',
          'computer-use',
          'mcp-server-full',
          'codegraph-full',
          'agent-memory',
          'long-context-1M',
          'multi-agent-goal-mode',
          'unlimited-file-upload',
          'plugin-allowlist-override',
          'shell-unrestricted-exec',
        ],
      },
    },
    features: [
      { label: t('feature_full_sandbox'), included: true },
      { label: t('feature_full_browser'), included: true },
      { label: t('feature_computer_use'), included: true },
      { label: t('feature_full_mcp'), included: true },
      { label: t('feature_full_codegraph'), included: true },
      { label: t('feature_full_memory'), included: true },
      { label: t('feature_1m'), included: true },
      { label: t('feature_multi_agent'), included: true },
      { label: t('feature_unlimited_upload'), included: true },
      { label: t('feature_plugin_override'), included: true },
      { label: t('feature_shell_unrestricted'), included: true },
    ],
  },
];

function detectActivePlan(content: string): string {
  try {
    const data = JSON.parse(content);
    return data?.subscription?.plan || 'unknown';
  } catch {
    return 'unknown';
  }
}

export default function Auth() {
  const { token } = theme.useToken();
  const { t } = useI18n();
  const { data: authContent, refetch } = useAuth();
  const [activePlan, setActivePlan] = useState<string>('unknown');
  const [applying, setApplying] = useState<string | null>(null);

  useEffect(() => {
    if (authContent) setActivePlan(detectActivePlan(authContent));
  }, [authContent]);

  const handleApply = async (plan: Plan) => {
    setApplying(plan.id);
    try {
      await writeAuthFile(JSON.stringify(plan.json, null, 2));
      setActivePlan(plan.id);
      message.success(t('switched_plan', { plan: plan.name }));
      await refetch();
    } catch (err: any) {
      message.error(err?.message || t('write_failed'));
    } finally {
      setApplying(null);
    }
  };

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      {/* Header */}
      <Card className="hover-card" style={{ borderRadius: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Space>
            <SafetyOutlined style={{ fontSize: 22, color: token.colorSuccess }} />
            <div>
              <Title level={5} style={{ margin: 0 }}>{t('auth_title')}</Title>
              <Text type="secondary" style={{ fontSize: 13 }}>
                {t('auth_desc').split('\\n')[0]}
                {t('auth_desc').split('\\n')[1]}
              </Text>
            </div>
          </Space>
          <Tag color={activePlan === 'unknown' ? 'default' : 'green'} style={{ fontSize: 12 }}>
            {t('current_plan', { plan: activePlan === 'unknown' ? t('not_detected') : activePlan.toUpperCase() })}
          </Tag>
        </div>
      </Card>

      {/* Plan cards */}
      <Row gutter={[12, 12]}>
        {planDefs(t).map(plan => (
          <Col xs={24} sm={12} lg={6} key={plan.id}>
            <Card
              className="hover-card"
              style={{
                borderRadius: 12, height: '100%',
                border: activePlan === plan.id ? `2px solid ${plan.color}` : undefined,
                position: 'relative', overflow: 'hidden',
              }}
              styles={{ body: { padding: 20, display: 'flex', flexDirection: 'column', height: '100%' } }}
            >
              {/* Active badge */}
              {activePlan === plan.id && (
                <div style={{
                  position: 'absolute', top: 8, right: -24,
                  background: plan.color, color: '#fff',
                  padding: '2px 28px', fontSize: 10, fontWeight: 600,
                  transform: 'rotate(45deg)',
                }}>
                  ACTIVE
                </div>
              )}

              {/* Plan header */}
              <div style={{ textAlign: 'center', marginBottom: 16 }}>
                <div style={{
                  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                  width: 48, height: 48, borderRadius: 12,
                  background: `${plan.color}20`, color: plan.color,
                  fontSize: 24, marginBottom: 8,
                }}>
                  {plan.icon}
                </div>
                <Title level={4} style={{ margin: 0, fontSize: 18 }}>{plan.name}</Title>
                <Text type="secondary" style={{ fontSize: 12 }}>{plan.description}</Text>
              </div>

              {/* Features list */}
              <div style={{ flex: 1, marginBottom: 16 }}>
                {plan.features.map(f => (
                  <div key={f.label} style={{
                    display: 'flex', alignItems: 'center', gap: 8,
                    padding: '4px 0', fontSize: 12,
                    color: f.included ? 'inherit' : 'var(--text-muted)',
                  }}>
                    {f.included
                      ? <CheckCircleFilled style={{ color: plan.color, fontSize: 13 }} />
                      : <CloseCircleOutlined style={{ color: 'var(--text-muted)', fontSize: 13 }} />
                    }
                    {f.label}
                  </div>
                ))}
              </div>

              {/* Apply button */}
              <Button
                type={activePlan === plan.id ? 'default' : 'primary'}
                ghost={activePlan === plan.id}
                onClick={() => handleApply(plan)}
                loading={applying === plan.id}
                style={{
                  width: '100%', borderRadius: 8,
                  borderColor: plan.color, color: activePlan === plan.id ? plan.color : undefined,
                  background: activePlan !== plan.id ? plan.color : undefined,
                }}
              >
                {activePlan === plan.id ? t('current_version') : t('switch_version')}
              </Button>
            </Card>
          </Col>
        ))}
      </Row>

      {/* Usage tip */}
      <Card style={{ borderRadius: 12, background: token.colorPrimaryBg, border: '1px solid ' + token.colorPrimary + '44' }}>
        <Space>
          <ThunderboltOutlined style={{ color: "#22c55e", fontSize: 16 }} />
          <Text style={{ fontSize: 13 }}>
            {t('auth_tip').split('\\n')[0]}
            {t('auth_tip').split('\\n')[1]}
          </Text>
        </Space>
      </Card>
    </Space>
  );
}
