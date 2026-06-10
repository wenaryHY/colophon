/**
 * Colophon Icons — Lucide React 统一导出
 * 所有图标集中管理，方便统一调整 size / strokeWidth / className
 */
import {
  FileText, FolderOpen, Folder, Tag, MessageSquare,
  Upload, Settings, User, LogOut,
  Plus, Pencil, Trash2, Eye,
  CheckCircle, XCircle, AlertCircle, Ban,
  Search, Filter, ChevronLeft, ChevronRight,
  ExternalLink, Loader2, X, Home,
  LayoutDashboard, BarChart3, Users, Shield,
  Image, File, Archive, MoreHorizontal,
  Copy, RefreshCw, Zap, Star, Clock, Edit, Edit2, Palette, Package, Webhook,
  ArrowLeft, Key, Menu, Minus, Database,
  type LucideProps
} from 'lucide-react';

// 默认图标属性：size 18, stroke-width 2
const defaultProps: LucideProps = { size: 18, strokeWidth: 2 };

/** 带默认属性的图标工厂 */
function icon(Comp: React.FC<LucideProps>, props?: LucideProps) {
  return (override?: LucideProps) => <Comp {...defaultProps} {...props} {...override} />;
}

// === 导航图标 ===
export const IconFileText = icon(FileText);
export const IconFolderOpen = icon(FolderOpen);
export const IconFolder = icon(Folder);
export const IconEdit2 = icon(Edit2);
export const IconTag = icon(Tag);
export const IconMessageSquare = icon(MessageSquare);
export const IconUpload = icon(Upload);
export const IconSettings = icon(Settings);

// === 用户相关 ===
export const IconUser = icon(User);
export const IconLogOut = icon(LogOut, { size: 16 });

// === 操作按钮 ===
export const IconPlus = icon(Plus, { size: 16 });
export const IconPencil = icon(Pencil, { size: 14 });
export const IconTrash2 = icon(Trash2, { size: 14 });
export const IconEye = icon(Eye, { size: 14 });
export const IconExternalLink = icon(ExternalLink, { size: 14 });

// === 状态图标 ===
export const IconCheckCircle = icon(CheckCircle);
export const IconXCircle = icon(XCircle);
export const IconAlertCircle = icon(AlertCircle);

// === UI 辅助 ===
export const IconSearch = icon(Search, { size: 16 });
export const IconFilter = icon(Filter, { size: 16 });
export const IconChevronLeft = icon(ChevronLeft, { size: 16 });
export const IconArrowLeft = icon(ArrowLeft, { size: 18 });
export const IconChevronRight = icon(ChevronRight, { size: 16 });
export const IconX = icon(X, { size: 18 });
export const IconLoader2 = icon(Loader2);
export const IconHome = icon(Home);
export const IconLayoutDashboard = icon(LayoutDashboard);
export const IconBarChart3 = icon(BarChart3);
export const IconUsers = icon(Users);
export const IconShield = icon(Shield);
export const IconImage = icon(Image);
export const IconFileIcon = icon(File);
export const IconArchive = icon(Archive);
export const IconMoreHorizontal = icon(MoreHorizontal);
export const IconCopy = icon(Copy, { size: 14 });
export const IconRefreshCw = icon(RefreshCw, { size: 14 });
export const IconZap = icon(Zap);
export const IconStar = icon(Star);
export const IconEdit = icon(Edit, { size: 14 });
export const IconClock = icon(Clock, { size: 14 });
export const IconBan = icon(Ban, { size: 16 });
export const IconPalette = icon(Palette);
export const IconPackage = icon(Package);
export const IconWebhook = icon(Webhook);
export const IconKey = icon(Key);
export const IconMenu = icon(Menu, { size: 20 });
export const IconMinus = icon(Minus, { size: 20 });
export const IconDatabase = icon(Database);

// === Spinner 动画图标 ===
export function Spinner({ className = '', size = 20 }: { className?: string; size?: number }) {
  return <Loader2 className={`animate-spin ${className}`} size={size} />;
}

// === 批量操作图标 ===
export const IconCheckbox = icon(File, { size: 16 });
export const IconSquare = icon(File, { size: 16 });
export const IconCheck = icon(CheckCircle, { size: 16 });
