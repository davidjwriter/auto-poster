// src/components/PostForm.js
import React, { useState } from 'react';
import axios from 'axios';

const PostForm = () => {
  const [post, setPost] = useState('');

  const handleSubmit = (e) => {
    e.preventDefault();
    axios.post('https://4zkgjocsu8.execute-api.us-east-1.amazonaws.com/prod/add', { post })
      .then(response => {
        console.log('Post added', response.data);
        setPost('');
      })
      .catch(error => {
        console.error('Error adding post', error);
      });
  };

  return (
    <form onSubmit={handleSubmit} className="post-form">
      <h2>Add New Post</h2>
      <input
        type="text"
        value={post}
        onChange={(e) => setPost(e.target.value)}
        placeholder="Enter your post"
      />
      <button type="submit">Add Post</button>
    </form>
  );
};

export default PostForm;
