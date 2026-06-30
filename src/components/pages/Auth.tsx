import { useState, useEffect } from 'react';
import { Card, Button, Typography, Space, Tag, message, Row, Col, theme } from 'antd';
import {
  CrownOutlined, StarOutlined, SafetyOutlined, ThunderboltOutlined,
  CheckCircleFilled, CloseCircleOutlined,
} from '@ant-design/icons';
import { useAuth } from '../../hooks/useApi';
import { writeAuthFile } from '../../api';

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

const PLANS: Plan[] = [
  {
    id: 'free',
    name: 'Free',
    icon: <SafetyOutlined />,
    color: '#6b7280',
    badgeColor: '#4b5563',
    description: '免费基础版 — 安全沙箱，轻量体验',
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
      { label: '安全沙箱', included: true },
      { label: '代码图谱精简版', included: true },
      { label: '自动化浏览器', included: false },
      { label: 'MCP 服务', included: false },
      { label: '电脑操控（Computer Use）', included: false },
      { label: '智能体记忆', included: false },
      { label: '百万上下文', included: false },
    ],
  },
  {
    id: 'plus',
    name: 'Plus',
    icon: <StarOutlined />,
    color: "#22c55e",
    badgeColor: '#16a34a',
    description: '个人会员版 — 基础执行 + 网页抓取',
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
      { label: '基础代码执行沙箱', included: true },
      { label: '基础网页抓取', included: true },
      { label: '基础 MCP 服务', included: true },
      { label: '代码图谱精简版', included: true },
      { label: '128K 上下文', included: true },
      { label: '电脑操控（Computer Use）', included: false },
      { label: '百万上下文', included: false },
    ],
  },
  {
    id: 'pro',
    name: 'Pro',
    icon: <CrownOutlined />,
    color: '#f59e0b',
    badgeColor: '#d97706',
    description: '高阶专业版 — 无限制沙箱，全自动化',
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
      { label: 'danger-full-access 无限制沙箱', included: true },
      { label: '浏览器全自动化', included: true },
      { label: '电脑操控（Computer Use）', included: true },
      { label: '完整 MCP 服务', included: true },
      { label: '完整代码图谱', included: true },
      { label: '智能体记忆', included: true },
      { label: '百万上下文（1M）', included: true },
    ],
  },
  {
    id: 'enterprise',
    name: 'Enterprise',
    icon: <ThunderboltOutlined />,
    color: '#ef4444',
    badgeColor: '#dc2626',
    description: '企业终极版 — 无限额度，无限制权限',
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
      { label: 'danger-full-access 无限制沙箱', included: true },
      { label: '浏览器全自动化', included: true },
      { label: '电脑操控（Computer Use）', included: true },
      { label: '完整 MCP 服务', included: true },
      { label: '完整代码图谱', included: true },
      { label: '智能体记忆', included: true },
      { label: '百万上下文（1M）', included: true },
      { label: '多任务目标模式', included: true },
      { label: '无限制文件上传', included: true },
      { label: '插件白名单解除', included: true },
      { label: '无限制 Shell 执行', included: true },
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
  // const { t } = useI18n();
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
      message.success(`已切换至 ${plan.name} 版，启用代理后将自动应用。`);
      await refetch();
    } catch (err: any) {
      message.error(err?.message || '写入失败');
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
              <Title level={5} style={{ margin: 0 }}>认证等级切换</Title>
              <Text type="secondary" style={{ fontSize: 13 }}>
                切换认证等级后，启用代理时将自动写入 Codex 配置。
                所有认证令牌均为模拟令牌，仅供本地测试使用。
              </Text>
            </div>
          </Space>
          <Tag color={activePlan === 'unknown' ? 'default' : 'green'} style={{ fontSize: 12 }}>
            当前: {activePlan === 'unknown' ? '未检测' : activePlan.toUpperCase()}
          </Tag>
        </div>
      </Card>

      {/* Plan cards */}
      <Row gutter={[12, 12]}>
        {PLANS.map(plan => (
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
                {activePlan === plan.id ? '当前版本' : '切换至此版本'}
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
            切换步骤：① 点击上方版本卡片 → ② 启用代理 → ③ 认证自动写入 ~/.codex/auth.json。
            切换版本或关闭代理时将自动还原原始配置。
          </Text>
        </Space>
      </Card>
    </Space>
  );
}
