//! Gluon v2 Matcher Performance Benchmarks
//!
//! This benchmark suite measures the performance improvements from Faza 1 & 2:
//! - Exact hash matching (O(1))
//! - Normalized tokenized matching (O(N))
//! - Rayon parallelization in FuzzyMatcher
//! - UCS validation in StructuralMatcher
//!
//! Run with: cargo bench --bench matcher_benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use gluon_desktop_lib::apply_system::matchers::{
    exact_matcher::ExactMatcher,
    normalized_matcher::NormalizedMatcher,
    fuzzy_matcher::FuzzyMatcher,
    weighted_anchor_matcher::WeightedAnchorMatcher,
    block_matcher::BlockMatcher,
    Matcher,
};

/// Sample JavaScript file for benchmarking (realistic size ~100 lines)
const SAMPLE_JS_FILE: &str = r#"
// React component with hooks
import React, { useState, useEffect } from 'react';
import { Button, Card, Modal } from './components';
import { fetchUserData, updateProfile } from './api';

export function UserProfile({ userId }) {
  const [user, setUser] = useState(null);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    async function loadUser() {
      try {
        const data = await fetchUserData(userId);
        setUser(data);
      } catch (error) {
        console.error('Failed to load user:', error);
      } finally {
        setLoading(false);
      }
    }

    loadUser();
  }, [userId]);

  const handleSave = async (updates) => {
    try {
      await updateProfile(userId, updates);
      setUser({ ...user, ...updates });
      setEditing(false);
    } catch (error) {
      alert('Failed to update profile');
    }
  };

  if (loading) {
    return <div>Loading...</div>;
  }

  return (
    <Card className="user-profile">
      <div className="header">
        <h2>{user.name}</h2>
        <Button onClick={() => setEditing(true)}>Edit</Button>
      </div>

      <div className="details">
        <p>Email: {user.email}</p>
        <p>Role: {user.role}</p>
        <p>Member since: {user.joinDate}</p>
      </div>

      {editing && (
        <Modal onClose={() => setEditing(false)}>
          <ProfileEditForm
            user={user}
            onSave={handleSave}
            onCancel={() => setEditing(false)}
          />
        </Modal>
      )}
    </Card>
  );
}

function ProfileEditForm({ user, onSave, onCancel }) {
  const [formData, setFormData] = useState({
    name: user.name,
    email: user.email,
  });

  const handleChange = (field, value) => {
    setFormData({ ...formData, [field]: value });
  };

  return (
    <form onSubmit={(e) => {
      e.preventDefault();
      onSave(formData);
    }}>
      <input
        type="text"
        value={formData.name}
        onChange={(e) => handleChange('name', e.target.value)}
      />
      <input
        type="email"
        value={formData.email}
        onChange={(e) => handleChange('email', e.target.value)}
      />
      <Button type="submit">Save</Button>
      <Button type="button" onClick={onCancel}>Cancel</Button>
    </form>
  );
}
"#;

/// Search block: exact match scenario
const SEARCH_EXACT: &str = r#"  const handleSave = async (updates) => {
    try {
      await updateProfile(userId, updates);
      setUser({ ...user, ...updates });
      setEditing(false);
    } catch (error) {
      alert('Failed to update profile');
    }
  };"#;

/// Search block: with formatting differences (spaces, newlines)
const SEARCH_FORMATTED: &str = r#"const handleSave=async(updates)=>{
try{
await updateProfile(userId,updates);
setUser({...user,...updates});
setEditing(false);
}catch(error){
alert('Failed to update profile');
}
};"#;

/// Search block: with comment differences
const SEARCH_WITH_COMMENTS: &str = r#"  const handleSave = async (updates) => {
    // Save user profile changes
    try {
      await updateProfile(userId, updates);
      // Update local state
      setUser({ ...user, ...updates });
      setEditing(false);
    } catch (error) {
      // Show error to user
      alert('Failed to update profile');
    }
  };"#;

fn bench_exact_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExactMatcher");

    group.bench_function("exact_match", |b| {
        let matcher = ExactMatcher;
        b.iter(|| {
            matcher.find_match(
                black_box(SAMPLE_JS_FILE),
                black_box(SEARCH_EXACT),
                Some("test.js")
            )
        });
    });

    group.bench_function("exact_match_miss", |b| {
        let matcher = ExactMatcher;
        b.iter(|| {
            matcher.find_match(
                black_box(SAMPLE_JS_FILE),
                black_box(SEARCH_FORMATTED), // Won't match exact
                Some("test.js")
            )
        });
    });

    group.finish();
}

fn bench_normalized_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("NormalizedMatcher");

    group.bench_function("match_with_formatting_diff", |b| {
        let matcher = NormalizedMatcher;
        b.iter(|| {
            matcher.find_match(
                black_box(SAMPLE_JS_FILE),
                black_box(SEARCH_FORMATTED),
                Some("test.js")
            )
        });
    });

    group.bench_function("match_with_comments", |b| {
        let matcher = NormalizedMatcher;
        b.iter(|| {
            matcher.find_match(
                black_box(SAMPLE_JS_FILE),
                black_box(SEARCH_WITH_COMMENTS),
                Some("test.js")
            )
        });
    });

    group.finish();
}

fn bench_fuzzy_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("FuzzyMatcher");

    // Measure impact of rayon parallelization
    group.bench_function("fuzzy_match_parallel", |b| {
        let matcher = FuzzyMatcher::new();
        b.iter(|| {
            matcher.find_match(
                black_box(SAMPLE_JS_FILE),
                black_box(SEARCH_EXACT),
                Some("test.js")
            )
        });
    });

    group.finish();
}

fn bench_cascade_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cascade");

    // Measure early exit optimization
    group.bench_function("exact_wins_immediately", |b| {
        b.iter(|| {
            // ExactMatcher should win on first try
            let exact = ExactMatcher;
            if exact.find_match(black_box(SAMPLE_JS_FILE), black_box(SEARCH_EXACT), Some("test.js")).is_some() {
                return; // Early exit
            }

            // Should never reach here
            let normalized = NormalizedMatcher;
            let _ = normalized.find_match(black_box(SAMPLE_JS_FILE), black_box(SEARCH_EXACT), Some("test.js"));
        });
    });

    group.bench_function("normalized_after_exact_fails", |b| {
        b.iter(|| {
            let exact = ExactMatcher;
            if exact.find_match(black_box(SAMPLE_JS_FILE), black_box(SEARCH_FORMATTED), Some("test.js")).is_some() {
                return;
            }

            // NormalizedMatcher should win here
            let normalized = NormalizedMatcher;
            let _ = normalized.find_match(black_box(SAMPLE_JS_FILE), black_box(SEARCH_FORMATTED), Some("test.js"));
        });
    });

    group.finish();
}

fn bench_file_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("FileSize Scaling");

    // Generate files of different sizes
    let small_file = SAMPLE_JS_FILE; // ~100 lines
    let medium_file = SAMPLE_JS_FILE.repeat(5); // ~500 lines
    let large_file = SAMPLE_JS_FILE.repeat(20); // ~2000 lines

    for (size_name, file_content) in [
        ("100_lines", small_file),
        ("500_lines", medium_file.as_str()),
        ("2000_lines", large_file.as_str()),
    ] {
        group.bench_with_input(BenchmarkId::new("ExactMatcher", size_name), &file_content, |b, content| {
            let matcher = ExactMatcher;
            b.iter(|| matcher.find_match(black_box(content), black_box(SEARCH_EXACT), Some("test.js")));
        });

        group.bench_with_input(BenchmarkId::new("FuzzyMatcher", size_name), &file_content, |b, content| {
            let matcher = FuzzyMatcher::new();
            b.iter(|| matcher.find_match(black_box(content), black_box(SEARCH_EXACT), Some("test.js")));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_exact_matcher,
    bench_normalized_matcher,
    bench_fuzzy_matcher,
    bench_cascade_performance,
    bench_file_sizes,
);
criterion_main!(benches);
