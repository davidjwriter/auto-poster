// src/App.js
import React from 'react';
import './App.css';
import EditablePosts from './components/EditablePosts';
import PostForm from './components/PostForm';
import ScheduledPostForm from './components/ScheduledPostForm';

function App() {
  return (
    <div className="App">
      <header className="App-header">
        <h1>Post Management App</h1>
      </header>
      <main>
        <PostForm />
        <ScheduledPostForm />
        <EditablePosts />
      </main>
    </div>
  );
}

export default App;
