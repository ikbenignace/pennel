import React, { useState } from 'react';
import { useRouter } from 'expo-router';
import { View } from 'react-native';
import type { ChatTurn, MeetingAnswer } from '../domain';
import { useRuntime } from '../runtime';
import { meetingAnswer } from '../normalize';
import { rememberSource } from '../source-navigation';
import { Body, Button, Field, Row, Section } from './ui';

export interface AskSession { question: string; turns: { question: string; answer: MeetingAnswer }[] }
export function AskPanel({ meetingId, session, onSessionChange }: { meetingId?: string; session?: AskSession; onSessionChange?: React.Dispatch<React.SetStateAction<AskSession>> }) {
  const { run, busy, t, theme } = useRuntime();
  const [local, setLocal] = useState<AskSession>({ question: '', turns: [] });
  const { question, turns } = session ?? local;
  const change = onSessionChange ?? setLocal;
  const setQuestion = (question: string) => change((previous) => ({ ...previous, question }));
  const router = useRouter();
  const key = `ask-${meetingId ?? 'all'}`;
  const ask = async () => {
    const text = question.trim();
    if (!text || busy.has(key)) return;
    const history: ChatTurn[] = turns.map((turn) => ({ question: turn.question, answer: turn.answer.text }));
    const result = await run<MeetingAnswer>(key, 'meetings.ask', { question: text, history, meeting_id: meetingId ?? null });
    if (result) change((previous) => ({ question: previous.question === question ? '' : previous.question, turns: [...previous.turns, { question: text, answer: meetingAnswer(result) }] }));
  };
  return <>
    {turns.map((turn, index) => <Section key={index} title={turn.question}><Body selectable>{turn.answer.text}</Body>{turn.answer.citations.map((citation) => <Row key={citation.number} title={`${citation.number}. ${citation.passage.title}`} subtitle={t(`meetings.source_${citation.passage.kind}`)} onPress={() => router.push({ pathname: '/meeting/[id]', params: { id: citation.passage.meeting_id, source: rememberSource(citation.passage) } })} />)}</Section>)}
    <View style={{ gap: theme.space[3] }}><Field label={t('meetings.your_question')} placeholder={t(meetingId ? 'meetings.ask_placeholder_this' : 'meetings.ask_placeholder_all')} value={question} onChangeText={setQuestion} multiline style={{ minHeight: 88 }} returnKeyType="default" /><Button title={t(busy.has(key) ? 'meetings.writing_answer' : 'meetings.ask')} icon="arrow-up" busy={busy.has(key)} disabled={!question.trim()} onPress={() => void ask()} /></View>
  </>;
}
