// src/components/EditablePosts.js
import React, { useEffect, useState, useRef } from 'react';
import axios from 'axios';
import './EditablePosts.css'; // Import the CSS file

const EditablePosts = () => {
  const [posts, setPosts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const fetchPosts = () => {
    axios.get('https://4zkgjocsu8.execute-api.us-east-1.amazonaws.com/prod/getPosts')
      .then(response => {
        console.log(response.data);
        // Ensure the data has a posts array
        if (response.data && Array.isArray(response.data.posts)) {
          setPosts(response.data.posts);
        } else {
          console.error('Fetched data does not contain a posts array:', response.data);
          setError('Unexpected data format');
        }
        setLoading(false);
      })
      .catch(error => {
        setError(error);
        setLoading(false);
      });
  };

  useEffect(() => {
    fetchPosts();
  }, []);

  const handleChange = (uuid, value) => {
    setPosts(posts.map(post => post.uuid === uuid ? { ...post, post: value } : post));
  };

  const handleSave = (uuid, value) => {
    axios.post('https://4zkgjocsu8.execute-api.us-east-1.amazonaws.com/prod/editPosts', { posts: [{ uuid, post: value }] })
      .then(response => {
        console.log(`Post ${uuid} saved`);
      })
      .catch(error => {
        console.error(`Error saving post ${uuid}`, error);
      });
  };

  const handleGenerate = () => {
    setLoading(true); // Set loading state to true while generating new posts
    axios.post('https://vl3wcjl6hk.execute-api.us-east-1.amazonaws.com/prod/generate')
      .then(response => {
        console.log('Generated new posts');
        return new Promise(resolve => setTimeout(resolve, 30000)); // Wait for 30 seconds
      })
      .then(() => fetchPosts()) // Fetch new posts after the timeout
      .catch(error => {
        console.error('Error generating posts', error);
        setLoading(false); // Reset loading state if there is an error
      });
  };

  const autoResizeTextarea = (element) => {
    element.style.height = 'auto';
    element.style.height = `${element.scrollHeight}px`;
  };

  useEffect(() => {
    posts.forEach(post => {
      const textarea = document.getElementById(post.uuid);
      if (textarea) {
        autoResizeTextarea(textarea);
      }
    });
  }, [posts]);

  if (loading) return <p>Loading...</p>;
  if (error) return <p>Error loading posts: {error.toString()}</p>;

  return (
    <div>
      <h2>Editable Posts</h2>
      {Array.isArray(posts) && posts.map(post => (
        <div key={post.uuid} className="post-item">
          <textarea
            id={post.uuid}
            value={post.post}
            onChange={(e) => {
              handleChange(post.uuid, e.target.value);
              autoResizeTextarea(e.target);
            }}
            className="post-textarea"
          />
          <button onClick={() => handleSave(post.uuid, post.post)}>Save</button>
        </div>
      ))}
      <button onClick={handleGenerate}>Generate New Posts</button>
    </div>
  );
};

export default EditablePosts;
