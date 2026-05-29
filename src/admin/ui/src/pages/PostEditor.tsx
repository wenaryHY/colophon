import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { apiData, API, API_PREFIX, getQueryClient } from '../lib/api';
import { esc } from '../lib/utils';
import { SlotRenderer } from '../lib/slots';
import type { AdminPost, Category, Tag } from '../types';

import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Select } from '../components/Select';
import { Modal } from '../components/Modal';
import { MarkdownEditor } from '../components/MarkdownEditor';
import {
  IconFileText, IconPlus, IconPencil, IconArrowLeft, IconCheck
} from '../components/Icons';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

type PageEditMode = 'editor' | 'custom_html';

/** Render mode choice dialog state */
interface RenderModeChoice {
  resolve: (mode: 'editor' | 'custom_html') => void;
}

export default function PostEditor() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useI18n();
  const toast = useToast();

  const isEdit = !!id;
  const [saving, setSaving] = useState(false);
  const [post, setPost] = useState<AdminPost | null>(null);

  // 表单字段
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [excerpt, setExcerpt] = useState('');
  const [status, setStatus] = useState<'published' | 'draft'>('draft');
  const [categoryId, setCategoryId] = useState('');
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [contentType, setContentType] = useState<'post' | 'page'>('post');
  const [pageEditMode, setPageEditMode] = useState<PageEditMode>('editor');
  const [customHtmlFile, setCustomHtmlFile] = useState<File | null>(null);

  // 渲染模式选择弹窗
  const [renderModeChoice, setRenderModeChoice] = useState<RenderModeChoice | null>(null);

  // 加载文章详情（编辑模式）
  const { isLoading: postLoading } = useQuery({
    queryKey: ['post', id],
    queryFn: async () => {
      const p = await apiData<AdminPost>(`${API_PREFIX}/admin/posts/${id}`);
      setPost(p);
      setTitle(p.title || '');
      setContent(p.content_md || '');
      setExcerpt(p.excerpt || '');
      setStatus(p.status === 'published' ? 'published' : 'draft');
      setCategoryId(p.category_id || '');
      setSelectedTagIds(p.tags?.map((tag) => tag.id) || []);
      setContentType(p.content_type || 'post');
      setPageEditMode(p.page_render_mode === 'custom_html' ? 'custom_html' : 'editor');
      return p;
    },
    enabled: isEdit,
  });

  // 加载分类
  const { data: categories = [] } = useQuery({
    queryKey: ['categories'],
    queryFn: () => apiData<Category[]>(`${API_PREFIX}/categories`),
  });

  // 加载标签
  const { data: tags = [] } = useQuery({
    queryKey: ['tags'],
    queryFn: () => apiData<Tag[]>(`${API_PREFIX}/tags`),
  });

  function toggleTag(tagId: string) {
    setSelectedTagIds((prev) =>
      prev.includes(tagId) ? prev.filter((item) => item !== tagId) : [...prev, tagId]
    );
  }

  async function handleSave(chosenRenderMode?: 'editor' | 'custom_html') {
    if (!title.trim()) { toast(t('titleRequired'), 'error'); return; }
    setSaving(true);
    try {
      const isPage = contentType === 'page';
      const hasCustomHtml = !!(post?.custom_html_path || customHtmlFile);
      const hasMdContent = !!content.trim();

      // 确定渲染模式
      let renderMode: 'editor' | 'custom_html' = 'editor';
      if (isPage) {
        if (hasCustomHtml && hasMdContent) {
          if (chosenRenderMode) {
            renderMode = chosenRenderMode;
          } else {
            setSaving(false);
            const mode = await new Promise<'editor' | 'custom_html'>((resolve) => {
              setRenderModeChoice({ resolve });
            });
            setRenderModeChoice(null);
            setSaving(true);
            renderMode = mode;
          }
        } else if (hasCustomHtml) {
          renderMode = 'custom_html';
        } else {
          renderMode = 'editor';
        }
      }

      const body: Record<string, unknown> = {
        title: title.trim(),
        excerpt: excerpt.trim() || null,
        content_md: content,
        status,
        visibility: 'public',
        category_id: categoryId || null,
        content_type: contentType,
        allow_comment: contentType === 'post',
        pinned: false,
        page_render_mode: renderMode,
      };

      if (contentType === 'post') {
        body.tag_ids = selectedTagIds;
      }

      if (post?.custom_html_path && !customHtmlFile) {
        body.custom_html_path = post.custom_html_path;
      }

      if (isEdit && post?.id) {
        await apiData(`${API_PREFIX}/admin/posts/${post.id}`, { method: 'PATCH', body: JSON.stringify(body) });
      } else {
        await apiData(`${API_PREFIX}/admin/posts`, { method: 'POST', body: JSON.stringify(body) });
      }

      // 上传自定义HTML
      if (isPage && customHtmlFile) {
        const slug = post?.slug || title.trim().toLowerCase().replace(/[^a-z0-9\u4e00-\u9fff]+/g, '-').replace(/^-|-$/g, '');
        const fd = new FormData();
        fd.append('file', customHtmlFile);
        fd.append('slug', slug);
        const uploadRes = await fetch(`${API}${API_PREFIX}/admin/pages/upload`, {
          method: 'POST',
          body: fd,
          credentials: 'include',
        }).then(r => r.json());

        if (uploadRes.code !== 0) throw new Error(uploadRes.message || '上传失败');

        const postId = post?.id;
        if (postId) {
          await apiData(`${API_PREFIX}/admin/posts/${postId}`, {
            method: 'PATCH',
            body: JSON.stringify({
              custom_html_path: uploadRes.data.custom_html_path,
              page_render_mode: renderMode,
            }),
          });
        }
      }

      toast(t('saveSuccess'), 'success');
      getQueryClient().invalidateQueries({ queryKey: ['posts'] });
      navigate('/posts');
    } catch (error) {
      toast(error instanceof Error ? error.message : t('saveFailed'), 'error');
    } finally {
      setSaving(false);
    }
  }

  if (postLoading) {
    return (
      <div style={{ padding: '40px', textAlign: 'center', color: 'var(--md-on-surface-variant)' }}>
        加载中…
      </div>
    );
  }

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* ── 顶部栏 ── */}
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '12px 0', marginBottom: '16px', flexShrink: 0,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <button
            onClick={() => navigate('/posts')}
            style={{
              display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
              width: '40px', height: '40px', borderRadius: 'var(--radius-full)',
              border: 'none', background: 'var(--md-surface-container)',
              color: 'var(--md-on-surface-variant)', cursor: 'pointer',
              transition: 'all var(--transition-normal)',
            }}
            onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-surface-container-high)'; }}
            onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container)'; }}
          >
            <IconArrowLeft size={18} />
          </button>
          <div>
            <h1 style={{
              fontSize: '20px', fontWeight: 800, color: 'var(--md-on-surface)',
              fontFamily: "'Manrope', sans-serif", letterSpacing: '-0.3px', lineHeight: 1.2,
            }}>
              {isEdit ? t('editPostTitle') : (contentType === 'page' ? '新建页面' : t('createPostTitle'))}
            </h1>
          </div>
        </div>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          {/* 内容类型切换（仅新建时） */}
          {!isEdit && (
            <div style={{
              display: 'inline-flex', gap: '0',
              background: 'var(--md-surface-container)', padding: '3px', borderRadius: 'var(--radius-full)',
            }}>
              <button
                onClick={() => setContentType('post')}
                style={{
                  padding: '6px 16px', borderRadius: 'var(--radius-full)',
                  border: 'none', cursor: 'pointer',
                  fontSize: '12.5px', fontWeight: contentType === 'post' ? 600 : 400,
                  background: contentType === 'post' ? 'var(--md-primary)' : 'transparent',
                  color: contentType === 'post' ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
                  transition: 'all var(--transition-fast)',
                  display: 'flex', alignItems: 'center', gap: '5px',
                }}
              >
                <IconFileText size={13} /> 文章
              </button>
              <button
                onClick={() => setContentType('page')}
                style={{
                  padding: '6px 16px', borderRadius: 'var(--radius-full)',
                  border: 'none', cursor: 'pointer',
                  fontSize: '12.5px', fontWeight: contentType === 'page' ? 600 : 400,
                  background: contentType === 'page' ? 'var(--md-primary)' : 'transparent',
                  color: contentType === 'page' ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
                  transition: 'all var(--transition-fast)',
                  display: 'flex', alignItems: 'center', gap: '5px',
                }}
              >
                <IconPencil size={13} /> 页面
              </button>
            </div>
          )}
          <Button variant="ghost" onClick={() => navigate('/posts')}>{t('cancel')}</Button>
          <Button onClick={() => handleSave()} disabled={saving} loading={saving}>
            <IconCheck size={14} /> {t('save')}
          </Button>
        </div>
      </div>

      {/* ── 编辑器主体 ── */}
      <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '1fr 260px', gap: '20px', minHeight: 0 }}>
        {/* 左侧：标题 + 编辑器 */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', minHeight: 0 }}>
          <Input
            label={t('titleLabel')}
            placeholder={t('titlePlaceholder')}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />

          {pageEditMode === 'editor' ? (
            <>
              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
                <div style={{ fontSize: '13px', fontWeight: 600, color: 'var(--md-on-surface-variant)', marginBottom: '6px' }}>
                  {t('postContentLabel')}
                </div>
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
                  <MarkdownEditor value={content} onChange={setContent} />
                </div>
              </div>
              <Input
                label={t('excerptLabel')}
                placeholder={t('excerptPlaceholder')}
                value={excerpt}
                onChange={(e) => setExcerpt(e.target.value)}
              />
            </>
          ) : (
            /* MD3 Drop zone */
            <div style={{
              border: '2px dashed var(--md-outline-variant)', borderRadius: 'var(--radius-lg)',
              padding: '40px 24px', textAlign: 'center',
              background: 'var(--md-surface-container-low)',
            }}>
              <div style={{
                width: '52px', height: '52px', margin: '0 auto 14px',
                borderRadius: '14px', background: 'var(--md-surface-container)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
              }}>
                <IconPlus size={24} style={{ color: 'var(--md-outline)' }} />
              </div>
              <p style={{ fontSize: '14px', fontWeight: 600, color: 'var(--md-on-surface)', marginBottom: '6px' }}>
                上传自定义 HTML
              </p>
              <p style={{ fontSize: '12.5px', color: 'var(--md-on-surface-variant)', marginBottom: '6px' }}>
                上传单个 HTML 文件或包含 HTML/CSS/JS 的 ZIP 包
              </p>
              <p style={{ fontSize: '11.5px', color: 'var(--md-outline)', marginBottom: '8px', lineHeight: 1.5 }}>
                ZIP 包中必须包含 index.html，发布后前台将通过 /pages/&#123;slug&#125; 访问
              </p>
              <p style={{ fontSize: '11.5px', color: 'var(--md-error)', marginBottom: '16px', lineHeight: 1.6 }}>
                发布后的自定义 HTML 会启用更严格的安全策略：禁止表单提交、外部连接与页面嵌入；如需脚本，仅应依赖当前站点内联或同源静态资源。
              </p>
              <input
                type="file"
                accept=".html,.htm,.zip"
                style={{ display: 'none' }}
                id="custom-html-upload-editor"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) setCustomHtmlFile(f);
                }}
              />
              <Button
                variant="ghost"
                onClick={() => document.getElementById('custom-html-upload-editor')?.click()}
              >
                选择文件
              </Button>
              {customHtmlFile && (
                <div style={{
                  marginTop: '12px', padding: '8px 14px', borderRadius: 'var(--radius-sm)',
                  background: 'var(--md-primary-container)',
                  fontSize: '12.5px', color: 'var(--md-on-primary-container)', fontWeight: 600,
                }}>
                  已选择: {customHtmlFile.name} ({Math.ceil(customHtmlFile.size / 1024)} KB)
                </div>
              )}
              {post?.custom_html_path && !customHtmlFile && (
                <div style={{
                  marginTop: '12px', padding: '8px 14px', borderRadius: 'var(--radius-sm)',
                  background: 'var(--md-surface-container)',
                  fontSize: '12px', color: 'var(--md-on-surface-variant)',
                }}>
                  当前自定义页面路径: {post.custom_html_path}
                  <br />重新上传将覆盖现有文件
                </div>
              )}
            </div>
          )}
        </div>

        {/* 右侧：发布设置 */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', overflowY: 'auto' }}>
          {/* 页面显示内容切换 — 仅页面类型 */}
          {(contentType === 'page') && (
            <div style={{
              background: 'var(--md-secondary-container)',
              borderRadius: 'var(--radius-lg)', padding: '20px',
            }}>
              <div style={{
                fontSize: '11.5px', fontWeight: 800, color: 'var(--md-on-secondary-container)',
                textTransform: 'uppercase', letterSpacing: '0.07em',
                marginBottom: '14px',
                display: 'flex', alignItems: 'center', gap: '6px',
              }}>
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--md-on-secondary-container)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
                  <line x1="9" y1="3" x2="9" y2="21"/>
                </svg>
                页面显示内容
              </div>
              <div style={{
                display: 'flex', gap: '0',
                background: 'var(--md-surface-container)', padding: '4px', borderRadius: 'var(--radius-full)',
              }}>
                <button
                  onClick={() => setPageEditMode('editor')}
                  style={{
                    flex: 1, padding: '7px 12px', borderRadius: 'var(--radius-full)',
                    border: 'none', cursor: 'pointer',
                    fontSize: '12.5px', fontWeight: pageEditMode === 'editor' ? 600 : 400,
                    background: pageEditMode === 'editor' ? 'var(--md-surface-container-lowest)' : 'transparent',
                    color: pageEditMode === 'editor' ? 'var(--md-on-surface)' : 'var(--md-on-surface-variant)',
                    transition: 'all var(--transition-fast)',
                  }}
                >
                  Markdown 编辑器
                </button>
                <button
                  onClick={() => setPageEditMode('custom_html')}
                  style={{
                    flex: 1, padding: '7px 12px', borderRadius: 'var(--radius-full)',
                    border: 'none', cursor: 'pointer',
                    fontSize: '12.5px', fontWeight: pageEditMode === 'custom_html' ? 600 : 400,
                    background: pageEditMode === 'custom_html' ? 'var(--md-surface-container-lowest)' : 'transparent',
                    color: pageEditMode === 'custom_html' ? 'var(--md-on-surface)' : 'var(--md-on-surface-variant)',
                    transition: 'all var(--transition-fast)',
                  }}
                >
                  自定义 HTML
                </button>
              </div>
              {post?.page_render_mode && (
                <div style={{
                  marginTop: '10px', padding: '6px 10px', borderRadius: 'var(--radius-full)',
                  background: post.page_render_mode === 'custom_html'
                    ? 'var(--md-secondary-container)' : 'var(--md-primary-container)',
                  fontSize: '11.5px', color: post.page_render_mode === 'custom_html'
                    ? 'var(--md-on-secondary-container)' : 'var(--md-on-primary-container)',
                  display: 'flex', alignItems: 'center', gap: '5px',
                }}>
                  <span style={{
                    width: '6px', height: '6px', borderRadius: '50%',
                    background: post.page_render_mode === 'custom_html' ? 'var(--md-secondary)' : 'var(--md-primary)',
                  }} />
                  当前前台显示：{post.page_render_mode === 'custom_html' ? '自定义 HTML' : 'Markdown 编辑器'}
                </div>
              )}
            </div>
          )}

          {/* 发布设置 */}
          <div style={{
            background: 'var(--md-surface-container)',
            borderRadius: 'var(--radius-lg)', padding: '20px',
          }}>
            <div style={{
              fontSize: '11.5px', fontWeight: 800, color: 'var(--md-on-surface-variant)',
              textTransform: 'uppercase', letterSpacing: '0.07em',
              marginBottom: '16px',
            }}>{t('publishSettings')}</div>
            <Select label={t('statusLabel')} value={status} onChange={(e) => setStatus(e.target.value as 'published' | 'draft')}>
              <option value="draft">{t('draftOption')}</option>
              <option value="published">{t('publishedOption')}</option>
            </Select>
          </div>

          {/* 分类和标签 */}
          <div style={{
            background: 'var(--md-surface-container)',
            borderRadius: 'var(--radius-lg)', padding: '20px',
          }}>
            <div style={{
              fontSize: '11.5px', fontWeight: 800, color: 'var(--md-on-surface-variant)',
              textTransform: 'uppercase', letterSpacing: '0.07em',
              marginBottom: '16px',
            }}>{t('categoryAndTags')}</div>
            <Select value={categoryId} onChange={(e) => setCategoryId(e.target.value)}>
              <option value="">{t('noCategory')}</option>
              {categories.map((cat) => (<option key={cat.id} value={cat.id}>{esc(cat.name)}</option>))}
            </Select>
            {contentType === 'post' && (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '7px', marginTop: '14px' }}>
                {tags.length > 0 ? tags.map((tag) => (
                  <button key={tag.id} type="button" onClick={() => toggleTag(tag.id)} style={{
                    border: 'none',
                    padding: '6px 14px', borderRadius: 'var(--radius-full)',
                    fontSize: '12px', fontWeight: 600, cursor: 'pointer',
                    background: selectedTagIds.includes(tag.id) ? 'var(--md-primary)' : 'var(--md-surface-container)',
                    color: selectedTagIds.includes(tag.id) ? 'var(--md-on-primary)' : 'var(--md-on-surface-variant)',
                    transition: 'all var(--transition-normal)',
                  }}>{esc(tag.name)}</button>
                )) : <span style={{ fontSize: '12px', color: 'var(--md-outline)' }}>{t('noTagsAvailable')}</span>}
              </div>
            )}

            <SlotRenderer target="post_editor.sidebar" context={id ? { post_id: id } : undefined} />
          </div>
        </div>
      </div>

      {/* 渲染模式选择弹窗 */}
      <Modal
        open={!!renderModeChoice}
        onClose={() => { renderModeChoice?.resolve('editor'); }}
        title="选择页面显示方式"
        width="480px"
        actions={
          <>
            <Button variant="ghost" onClick={() => { renderModeChoice?.resolve('editor'); }}>取消</Button>
          </>
        }
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <p style={{ fontSize: '14px', color: 'var(--md-on-surface-variant)', lineHeight: 1.6 }}>
            此页面同时有 Markdown 内容和自定义 HTML 内容，请选择前台访问者看到的版本：
          </p>
          <button
            onClick={() => { renderModeChoice?.resolve('editor'); }}
            style={{
              display: 'flex', alignItems: 'center', gap: '12px',
              padding: '16px 18px', borderRadius: 'var(--radius-lg)',
              border: 'none',
              background: 'var(--md-surface-container-lowest)', cursor: 'pointer',
              transition: 'all var(--transition-normal)', textAlign: 'left',
            }}
            onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-primary-container)'; }}
            onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-lowest)'; }}
          >
            <div style={{
              width: '40px', height: '40px', borderRadius: '12px',
              background: 'var(--md-primary-container)', display: 'flex',
              alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>
              <IconFileText size={20} style={{ color: 'var(--md-on-primary-container)' }} />
            </div>
            <div>
              <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--md-on-surface)' }}>使用 Markdown 编辑器</div>
              <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginTop: '2px' }}>通过主题模板渲染，样式统一</div>
            </div>
          </button>
          <button
            onClick={() => { renderModeChoice?.resolve('custom_html'); }}
            style={{
              display: 'flex', alignItems: 'center', gap: '12px',
              padding: '16px 18px', borderRadius: 'var(--radius-lg)',
              border: 'none',
              background: 'var(--md-surface-container-lowest)', cursor: 'pointer',
              transition: 'all var(--transition-normal)', textAlign: 'left',
            }}
            onMouseEnter={e => { e.currentTarget.style.background = 'var(--md-secondary-container)'; }}
            onMouseLeave={e => { e.currentTarget.style.background = 'var(--md-surface-container-lowest)'; }}
          >
            <div style={{
              width: '40px', height: '40px', borderRadius: '12px',
              background: 'var(--md-secondary-container)', display: 'flex',
              alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>
              <IconPencil size={20} style={{ color: 'var(--md-on-secondary-container)' }} />
            </div>
            <div>
              <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--md-on-surface)' }}>使用自定义 HTML</div>
              <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginTop: '2px' }}>完全自定义，独立于主题样式</div>
            </div>
          </button>
        </div>
      </Modal>
    </div>
  );
}
