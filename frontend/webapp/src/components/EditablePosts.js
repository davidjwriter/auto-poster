// src/components/EditablePosts.js
import React, { useEffect, useState } from 'react';
import axios from 'axios';

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
    axios.post('https://4zkgjocsu8.execute-api.us-east-1.amazonaws.com/prod/editPosts', { uuid, post: value })
      .then(response => {
        console.log(`Post ${uuid} saved`);
      })
      .catch(error => {
        console.error(`Error saving post ${uuid}`, error);
      });
  };

  const handleGenerate = () => {
    axios.post('https://vl3wcjl6hk.execute-api.us-east-1.amazonaws.com/prod/generate')
      .then(response => {
        console.log('Generated new posts');
        fetchPosts();
      })
      .catch(error => {
        console.error('Error generating posts', error);
      });
  };

  if (loading) return <p>Loading...</p>;
  if (error) return <p>Error loading posts: {error.toString()}</p>;

  return (
    <div>
      <h2>Editable Posts</h2>
      {Array.isArray(posts) && posts.map(post => (
        <div key={post.uuid} className="post-item">
          <input
            type="text"
            value={post.post}
            onChange={(e) => handleChange(post.uuid, e.target.value)}
          />
          <button onClick={() => handleSave(post.uuid, post.post)}>Save</button>
        </div>
      ))}
      <button onClick={handleGenerate}>Generate New Posts</button>
    </div>
  );
};

export default EditablePosts;
