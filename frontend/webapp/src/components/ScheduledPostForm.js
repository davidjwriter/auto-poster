// src/components/ScheduledPostForm.js
import React, { useState } from 'react';
import axios from 'axios';

const ScheduledPostForm = () => {
  const [post, setPost] = useState('');
  const [time, setTime] = useState('');
  const [recurring, setRecurring] = useState(false);

  const handleSubmit = (e) => {
    e.preventDefault();
    axios.post('https://4zkgjocsu8.execute-api.us-east-1.amazonaws.com/prod/addSchedule', { post, time, recurring })
      .then(response => {
        console.log('Scheduled post added', response.data);
        setPost('');
        setTime('');
        setRecurring(false);
      })
      .catch(error => {
        console.error('Error adding scheduled post', error);
      });
  };

  return (
    <form onSubmit={handleSubmit} className="scheduled-post-form">
      <h2>Add Scheduled Post</h2>
      <input
        type="text"
        value={post}
        onChange={(e) => setPost(e.target.value)}
        placeholder="Enter your post"
      />
      <input
        type="time"
        value={time}
        onChange={(e) => setTime(e.target.value)}
      />
      <label>
        <input
          type="checkbox"
          checked={recurring}
          onChange={(e) => setRecurring(e.target.checked)}
        />
        Recurring
      </label>
      <button type="submit">Add Scheduled Post</button>
    </form>
  );
};

export default ScheduledPostForm;
